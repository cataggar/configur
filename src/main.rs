use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(version)]
struct Cli {
    source: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let source = &cli.source;
    println!("source: {source}");
    let path = std::path::Path::new(&cli.source);
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
