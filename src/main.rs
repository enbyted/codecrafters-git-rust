use clap::{
    builder::{ValueParser, ValueParserFactory},
    Args, Parser,
};
use sha1::{Digest, Sha1};
use std::{fs, io::Read, io::Write, path::PathBuf};

#[derive(Parser)]
enum Subcommand {
    /// Initializes an empty repository
    Init,
    /// Reads a specific object from repository
    CatFile(CatFileArgs),
    /// Calculates hash for an object, optionally saves it to repository
    HashObject(HashObjectArgs),
    /// Print out contents of a tree object
    LsTree(LsTreeArgs),
}

#[derive(Debug, Clone, Args)]
struct CatFileArgs {
    /// The object hash to read out
    #[arg(required(true), index(1))]
    object: ObjectRef,
    /// Automatically pretty-print based on object type
    #[arg(short)]
    pretty_print: bool,
}

#[derive(Debug, Clone, Args)]
struct LsTreeArgs {
    /// The object hash to read out
    #[arg(required(true), index(1))]
    object: ObjectRef,
    /// Automatically pretty-print based on object type
    #[arg(long)]
    name_only: bool,
}

#[derive(Debug, Clone, Args)]
struct HashObjectArgs {
    /// The file to read data from
    #[arg(required(true), index(1))]
    file: String,
    /// Automatically pretty-print based on object type
    #[arg(short)]
    write: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ObjectRef(String);

impl ObjectRef {
    pub fn from_sha1(hash: &str) -> anyhow::Result<ObjectRef> {
        anyhow::ensure!(hash.len() == 40);
        anyhow::ensure!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        Ok(ObjectRef(hash.to_owned()))
    }

    fn hash_prefix(&self) -> &str {
        &self.0[..2]
    }

    fn matches_remainder(&self, remainder: &str) -> bool {
        remainder.eq_ignore_ascii_case(&self.0[2..])
    }

    fn matches(&self, hash: &str) -> bool {
        self.0.eq_ignore_ascii_case(hash)
    }
}

impl ValueParserFactory for ObjectRef {
    type Parser = ValueParser;

    fn value_parser() -> Self::Parser {
        ValueParser::from(ObjectRef::from_sha1)
    }
}

struct Repository {
    path: PathBuf,
}

impl Repository {
    pub fn from_current_dir() -> anyhow::Result<Repository> {
        Ok(Repository {
            path: std::env::current_dir()?.join(".git"),
        })
    }

    pub fn find_from_current_dir() -> anyhow::Result<Repository> {
        let mut current_dir = std::env::current_dir()?;
        // TODO: Arbitrary depth limit of 50, make it configurable
        for _ in 0..50 {
            let maybe_path = fs::read_dir(&current_dir)?.find_map(|d| {
                let dir = d.ok()?;
                if dir.file_type().ok()?.is_dir() && dir.file_name() == ".git" {
                    Some(dir.path())
                } else {
                    None
                }
            });

            if let Some(path) = maybe_path {
                return Ok(Repository { path });
            }
            if !current_dir.pop() {
                return Err(anyhow::Error::msg(
                    "Git repository not found (reached root)",
                ));
            }
        }
        Err(anyhow::Error::msg(format!(
            "Git repository not found (depth limit reached at {:?})",
            current_dir
        )))
    }

    pub fn init(&self) -> anyhow::Result<()> {
        fs::create_dir(&self.path)?;
        fs::create_dir(self.path.join("objects"))?;
        fs::create_dir(self.path.join("refs"))?;
        fs::write(self.path.join("HEAD"), "ref: refs/heads/master\n")?;
        Ok(())
    }

    pub fn find_object(&self, object_ref: &ObjectRef) -> anyhow::Result<Object> {
        let maybe_container_path = fs::read_dir(self.path.join("objects/"))?.find_map(|f| {
            let dir = f.ok()?;
            if dir.file_type().ok()?.is_dir()
                && dir
                    .file_name()
                    .eq_ignore_ascii_case(object_ref.hash_prefix())
            {
                Some(dir.path())
            } else {
                None
            }
        });

        if let Some(container_path) = maybe_container_path {
            let maybe_object_path = fs::read_dir(container_path)?.find_map(|f| {
                let file = f.ok()?;
                if file.file_type().ok()?.is_file()
                    && object_ref.matches_remainder(file.file_name().to_str()?)
                {
                    Some(file.path())
                } else {
                    None
                }
            });

            if let Some(object_path) = maybe_object_path {
                let object = Object::from_path(&object_path)?;
                anyhow::ensure!(object_ref.matches(&object.hash_string()));
                Ok(object)
            } else {
                Err(anyhow::Error::msg("Could not find requested object"))
            }
        } else {
            Err(anyhow::Error::msg("Could not find requested object"))
        }
    }

    pub fn save_object(&self, object: &Object) -> anyhow::Result<()> {
        let hash = object.hash_string();
        let (prefix, remainder) = hash.as_str().split_at(2);
        let container_path = self.path.join("objects/").join(prefix);
        let file_path = container_path.join(remainder);
        fs::create_dir_all(container_path)?;
        object.write_to(&file_path)
    }
}

#[derive(Debug, Clone)]
struct TreeItem<'a> {
    mode: u32,
    name: &'a str,
    hash: &'a [u8; 20],
}

impl<'a> TreeItem<'a> {
    pub fn parse<'s>(data: &'s [u8]) -> anyhow::Result<(&'s [u8], TreeItem<'s>)> {
        let index_first_zero = data.iter().position(|b| *b == 0u8).ok_or_else(|| {
            anyhow::Error::msg("Invalid tree item data, could not find filename terminator")
        })?;
        let (header, rest) = data.split_at(index_first_zero);
        let (hash, rest) = rest[1..].split_at(20);
        let (mode, name) = std::str::from_utf8(header)?
            .split_once(' ')
            .ok_or_else(|| {
                anyhow::Error::msg(
                    "Invalid tree item data, could not find split between mode and filename.",
                )
            })?;

        Ok((
            rest,
            TreeItem {
                mode: u32::from_str_radix(mode, 8)?,
                name,
                hash: hash.try_into()?,
            },
        ))
    }

    pub fn is_file(&self) -> bool {
        0 == (self.mode & (1 << 16))
    }
}

#[derive(Debug, Clone)]
struct TreeDataIterator<'a> {
    data: &'a [u8],
}

impl<'a> Iterator for TreeDataIterator<'a> {
    type Item = TreeItem<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let (rest, item) = TreeItem::parse(self.data).ok()?;
        self.data = rest;
        Some(item)
    }
}

#[derive(Debug, Clone)]
struct TreeData {
    data: Vec<u8>,
}

impl TreeData {
    pub fn iter(&self) -> TreeDataIterator<'_> {
        TreeDataIterator { data: &self.data }
    }
}

#[derive(Debug, Clone)]
enum Object {
    Unknown { kind: String, data: Vec<u8> },
    Blob(Vec<u8>),
    Commit(String),
    Tree(TreeData),
}

impl Object {
    fn from_path(path: &PathBuf) -> anyhow::Result<Object> {
        let file = fs::File::open(path)?;
        let mut decoder = flate2::read::ZlibDecoder::new(&file);
        let mut contents = Vec::new();
        decoder.read_to_end(&mut contents)?;

        // The data in contents is structured like this:
        // <type> <size>\0<payload>
        //       ^- a space character
        // Thus we need to split at first zero byte
        let index_first_zero = contents.iter().position(|b| *b == 0u8).ok_or_else(|| {
            anyhow::Error::msg("Invalid object file data, could not find header terminator.")
        })?;
        let header = std::str::from_utf8(&contents[..index_first_zero])?;
        let (object_type, object_size) = header.split_once(' ').ok_or_else(|| {
            anyhow::Error::msg(format!(
                "Invalid object file header format, expected '<type> <size>', found '{}'",
                header
            ))
        })?;
        let object_data = &contents[index_first_zero + 1..];
        let object_size = object_size.parse::<usize>()?;
        anyhow::ensure!(object_size == object_data.len());

        Ok(match object_type {
            "blob" => Object::Blob(object_data.to_owned()),
            "commit" => Object::Commit(std::str::from_utf8(object_data)?.to_owned()),
            "tree" => Object::Tree(TreeData {
                data: object_data.to_owned(),
            }),
            _ => Object::Unknown {
                kind: object_type.to_owned(),
                data: object_data.to_owned(),
            },
        })
    }

    pub fn kind(&self) -> &str {
        match self {
            Object::Blob(_) => "blob",
            Object::Commit(_) => "commit",
            Object::Tree(_) => "tree",
            Object::Unknown { kind, .. } => kind,
        }
    }

    pub fn contents_bytes(&self) -> &[u8] {
        match self {
            Object::Blob(data) => data,
            Object::Commit(text) => text.as_bytes(),
            Object::Tree(data) => &data.data,
            Object::Unknown { data, .. } => data,
        }
    }

    pub fn hash(&self) -> [u8; 20] {
        let mut hasher = Sha1::new();
        hasher.update(self.kind().as_bytes());
        hasher.update(b" ");
        hasher.update(self.contents_bytes().len().to_string().as_bytes());
        hasher.update(b"\0");
        hasher.update(&self.contents_bytes());
        hasher.finalize().into()
    }

    pub fn hash_string(&self) -> String {
        self.hash().iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn write_to(&self, path: &PathBuf) -> anyhow::Result<()> {
        let file = fs::File::create(path)?;
        let mut encoder = flate2::write::ZlibEncoder::new(file, flate2::Compression::fast());
        encoder.write_all(self.kind().as_bytes())?;
        encoder.write_all(b" ")?;
        encoder.write_all(self.contents_bytes().len().to_string().as_bytes())?;
        encoder.write_all(b"\0")?;
        encoder.write_all(&self.contents_bytes())?;
        encoder.flush()?;
        Ok(())
    }
}

fn cmd_init() -> anyhow::Result<()> {
    let repo = Repository::from_current_dir()?;
    println!("Creating git repository in {:?}", repo.path);
    repo.init()
}

fn cmd_cat_file(args: CatFileArgs) -> anyhow::Result<()> {
    let repo = Repository::find_from_current_dir()?;
    eprintln!("Git repository found in {:?}", repo.path);
    let obj = repo.find_object(&args.object)?;
    eprintln!("Found object: {:?}", obj);
    if args.pretty_print {
        match obj {
            Object::Blob(data) => std::io::stdout().lock().write_all(&data)?,
            Object::Commit(text) => print!("{text}"),
            Object::Tree(data) => {
                for item in data.iter() {
                    let mode = item.mode;
                    let kind = if item.is_file() { "blob" } else { "tree" };
                    let name = item.name;
                    let hash: String = item.hash.iter().map(|b| format!("{b:02x}")).collect();
                    println!("{mode:06o} {kind} {hash}    {name}");
                }
            }
            _ => anyhow::bail!("Don't know how to pretty-print {obj:?}."),
        }
    } else {
        std::io::stdout().lock().write_all(obj.contents_bytes())?;
    }
    Ok(())
}

fn cmd_ls_tree(args: LsTreeArgs) -> anyhow::Result<()> {
    let repo = Repository::find_from_current_dir()?;
    eprintln!("Git repository found in {:?}", repo.path);
    let obj = repo.find_object(&args.object)?;
    eprintln!("Found object: {:?}", obj);
    match obj {
        Object::Tree(data) => {
            for item in data.iter() {
                if args.name_only {
                    println!("{}", item.name);
                } else {
                    let mode = item.mode;
                    let kind = if item.is_file() { "blob" } else { "tree" };
                    let name = item.name;
                    let hash: String = item.hash.iter().map(|b| format!("{b:02x}")).collect();
                    println!("{mode:06o} {kind} {hash}    {name}");
                }
            }
        }
        _ => anyhow::bail!(
            "Not a tree object. {} is {}.",
            obj.hash_string(),
            obj.kind()
        ),
    }
    Ok(())
}

fn cmd_hash_object(args: HashObjectArgs) -> anyhow::Result<()> {
    let mut data = Vec::new();
    let mut file = fs::File::open(&args.file)?;
    file.read_to_end(&mut data)?;
    drop(file);

    let object = Object::Blob(data);
    println!("{}", object.hash_string());
    if args.write {
        let repo = Repository::find_from_current_dir()?;
        eprintln!("Git repository found in {:?}", repo.path);
        repo.save_object(&object)?;
    }
    Ok(())
}

fn main() {
    let res = match Subcommand::parse() {
        Subcommand::Init => cmd_init(),
        Subcommand::CatFile(args) => cmd_cat_file(args),
        Subcommand::LsTree(args) => cmd_ls_tree(args),
        Subcommand::HashObject(args) => cmd_hash_object(args),
    };

    if let Err(error) = res {
        eprintln!("Error during command execution: {:?}", error);
        panic!("Failed!");
    }
}
