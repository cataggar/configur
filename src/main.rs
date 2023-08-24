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
    let paths = path.read_dir();
    match paths {
        Ok(paths) => {
            let dir_entries = paths.into_iter().collect::<Vec<_>>();
            println!("# of dir entries: {}", dir_entries.len());
            for path in dir_entries {
                println!("dir entry: {path:?}");
            }
        }
        Err(e) => println!("{:?}", e),
    }
    Ok(())
}
