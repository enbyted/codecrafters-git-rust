use clap::{
    builder::{ValueParser, ValueParserFactory},
    Args, Parser,
};
use std::{fs, path::PathBuf};

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
        todo!("Search for .git directory in current dir or its parents");
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
    repo.init()
}

fn cmd_cat_file(args: CatFileArgs) -> anyhow::Result<()> {
    let _repo = Repository::find_from_current_dir()?;
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
