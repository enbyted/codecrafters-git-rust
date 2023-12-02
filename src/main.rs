use clap::{
    builder::{ValueParser, ValueParserFactory},
    Args, Parser,
};
use std::{
    fs::{self, FileType},
    path::PathBuf,
};

#[derive(Parser)]
enum Subcommand {
    /// Initializes an empty repository
    Init,
    /// Reads a specific object from repository
    CatFile(CatFileArgs),
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

#[derive(Debug, Clone, Eq, PartialEq)]
struct ObjectRef(String);

impl ObjectRef {
    pub fn from_sha1(hash: &str) -> anyhow::Result<ObjectRef> {
        anyhow::ensure!(hash.len() == 40);
        anyhow::ensure!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        Ok(ObjectRef(hash.to_owned()))
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
}

fn cmd_init() -> anyhow::Result<()> {
    let repo = Repository::from_current_dir()?;
    println!("Creating git repository in {:?}", repo.path);
    repo.init()
}

fn cmd_cat_file(args: CatFileArgs) -> anyhow::Result<()> {
    let repo = Repository::find_from_current_dir()?;
    println!("Git repository found in {:?}", repo.path);
    todo!("Search for object {:?}", args.object);
}

fn main() {
    let res = match Subcommand::parse() {
        Subcommand::Init => cmd_init(),
        Subcommand::CatFile(args) => cmd_cat_file(args),
    };

    if let Err(error) = res {
        eprintln!("Error during command execution: {:?}", error);
        panic!("Failed!");
    }
}
