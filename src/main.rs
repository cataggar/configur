use anyhow::Context;
use anyhow::Result;
use camino::*;
use clap::{Parser, Subcommand};
use glob::glob;
use serde_json::json;
use serde_json_merge::*;
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
    let exists = source.exists();
    println!("source: {source}, exists: {exists}");
    let source = source.to_string();
    let mut files = Vec::new();
    match glob(&format!("{source}/**/*.yml")) {
        Ok(paths) => {
            for path in paths.flatten() {
                match Utf8PathBuf::from_path_buf(path) {
                    Ok(path) => files.push(path),
                    Err(e) => println!("{:?}", e),
                }
            }
        }
        Err(e) => println!("{:?}", e),
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

            let dirs = dirs_files.keys();
            for dir in dirs {
                let dump_json_path = target.join(dir).join("dump2.json");
                println!("dump_json_path: {dump_json_path}");
                let mut dump_json = json!({});

                let mut input_yml_paths = Vec::new();
                let mut ancestors = dir.ancestors().into_iter().collect::<Vec<_>>();
                ancestors.reverse();
                for ancestor in ancestors {
                    if let Some(dir_files) = dirs_files.get(ancestor) {
                        for file in dir_files {
                            // println!("input file: {file}");
                            input_yml_paths.push(file);
                        }
                    }
                }
                for input_yml_path in input_yml_paths {
                    // println!("input_yml_path: {input_yml_path}");
                    let input_yml_path = source.join(input_yml_path);
                    let json: serde_json::Value =
                        serde_yaml::from_slice(&std::fs::read(&input_yml_path)?)
                            .with_context(|| format!("reading {input_yml_path}"))?;
                    dump_json = dump_json.merged_recursive::<Dfs>(&json);
                }
                dump_json.sort_keys();
                std::fs::write(&dump_json_path, serde_json::to_string_pretty(&dump_json)?)?;
            }
        }
        None => {}
    }
    Ok(())
}
