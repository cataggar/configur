use anyhow::Context;
use anyhow::Result;
use camino::*;
use clap::{Parser, Subcommand};
use glob::glob;
use std::{collections::BTreeMap, str::FromStr};

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

fn list_yml_files(source: &Utf8Path) -> Vec<Utf8PathBuf> {
    let source = source.to_string();
    let mut files = Vec::new();
    if let Ok(paths) = glob(&format!("{source}/**/*.yml")) {
        for path in paths {
            if let Ok(path) = path {
                if let Ok(path) = Utf8PathBuf::from_path_buf(path) {
                    files.push(path);
                }
            }
        }
    }
    files
}

fn group_yml_files_by_dir(files: Vec<&Utf8Path>) -> BTreeMap<Utf8PathBuf, Vec<&Utf8Path>> {
    let mut dirs: BTreeMap<Utf8PathBuf, Vec<&Utf8Path>> = BTreeMap::new();
    for file in files {
        let dir = file.parent();
        if let Some(dir) = dir {
            if let Some(files) = dirs.get_mut(dir) {
                files.push(file);
            } else {
                dirs.insert(dir.to_owned(), vec![file]);
            }
        }
    }
    dirs
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Some(Commands::Config { source, target }) => {
            println!("Printing source {source} and target {target}");

            let source = Utf8PathBuf::from_str(source)?;
            let target = Utf8PathBuf::from_str(target)?;

            let yml_files = list_yml_files(&source);
            let yml_files = yml_files
                .iter()
                .map(|x| x.strip_prefix(&source).with_context(|| "strip prefix"))
                .collect::<Result<Vec<_>>>()?;
            let dirs_files = group_yml_files_by_dir(yml_files);
            let dirs_files: BTreeMap<&Utf8Path, Vec<&Utf8Path>> = dirs_files
                .iter()
                .map(|(k, v)| (k.as_path(), v.clone()))
                .collect();
            for (dir, files) in &dirs_files {
                // let dir = dir.strip_prefix(&source)?;
                println!("\nDir: {dir}");
                for file in files {
                    // let file = file.strip_prefix(&source)?;
                    println!("File: {file}", file = file);
                }
            }

            let dirs = dirs_files.keys();
            for dir in dirs {
                // let dir = dir.strip_prefix(&source)?;
                println!("Dir: {dir}");
                let mut ancestors = dir.ancestors().into_iter().collect::<Vec<_>>();
                ancestors.reverse();
                for ancestor in ancestors {
                    println!("Ancestor: {ancestor}");
                    // let ancestor_full = source.join(ancestor);
                    // println!("Ancestor full: {ancestor_full}");
                    if let Some(dir_files) = dirs_files.get(ancestor) {
                        for file in dir_files {
                            // let file = file.strip_prefix(&source)?;
                            println!("Input file: {file}", file = file);
                        }
                    }
                }
            }
        }
        None => {}
    }
    Ok(())
}
