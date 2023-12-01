use clap::Parser;
use std::fs;

#[derive(Parser)]
enum Subcommand {
    /// Initializes an empty repository
    Init,
}

fn cmd_init() -> anyhow::Result<()> {
    fs::create_dir(".git")?;
    fs::create_dir(".git/objects")?;
    fs::create_dir(".git/refs")?;
    fs::write(".git/HEAD", "ref: refs/heads/master\n")?;
    Ok(())
}

fn main() {
    let res = match Subcommand::parse() {
        Subcommand::Init => cmd_init(),
    };

    if let Err(error) = res {
        eprintln!("Error during command execution: {:?}", error);
        panic!("Failed!");
    }
}
