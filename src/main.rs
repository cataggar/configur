use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Config { source: String, target: String },
}

fn main() -> Result<()> {
    let path = std::path::Path::new("/ev2");
    let exists = path.exists();
    println!("exists: {exists}");
    let is_dir = path.is_dir();
    println!("is_dir: {is_dir}");
    let files = path.read_dir();
    match files {
        Ok(paths) => {
            for path in paths {
                println!("file: {path:?}");
            }
        }
        Err(e) => println!("{:?}", e),
    }
    Ok(())
}
