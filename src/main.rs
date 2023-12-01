// use std::env;
// use std::fs;
use clap::Parser;

#[derive(Parser)]
enum Subcommand
{
    /// Initializes an empty repository
    Init,
}

fn main() {
    match Subcommand::parse() {
        Subcommand::Init => todo!(),
    };

    // Uncomment this block to pass the first stage
    // let args: Vec<String> = env::args().collect();
    // if args[1] == "init" {
    //     fs::create_dir(".git").unwrap();
    //     fs::create_dir(".git/objects").unwrap();
    //     fs::create_dir(".git/refs").unwrap();
    //     fs::write(".git/HEAD", "ref: refs/heads/master\n").unwrap();
    //     println!("Initialized git directory")
    // } else {
    //     println!("unknown command: {}", args[1])
    // }
}
