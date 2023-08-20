use clap::{Parser, Subcommand};
use glob::glob;

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Config {
        source: String,
        target: String,
    },
}

fn main() {
    let cli = Cli::parse();
    match &cli.command {
        Some(Commands::Config { source, target }) => {
            println!("Printing source {source} and target {target}");

            for entry in glob(&format!("{source}/**/*.yml")).expect("Failed to read glob pattern") {
                match entry {
                    Ok(path) => println!("{:?}", path.display()),
                    Err(e) => println!("{:?}", e),
                }
            }
        }
        None => {}
    }
}