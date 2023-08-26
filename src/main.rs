use anyhow::Context;
use anyhow::Result;
use camino::*;
use clap::Parser;
use glob::glob;
use serde_json::json;
use serde_json::Value;
use serde_json_merge::*;
use std::collections::HashMap;
use std::{collections::BTreeMap, fs, str::FromStr};

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[arg(long)]
    ev2: String,
    #[arg(short, long, default_value = "environments")]
    environments: String,
    #[arg(short, long, default_value = "scratch")]
    scratch: String,
}

fn list_yml_paths(dir: &Utf8Path) -> Vec<Utf8PathBuf> {
    let mut paths = Vec::new();
    match glob(&format!("{dir}/**/*.yml")) {
        Ok(glob_paths) => {
            for path in glob_paths.flatten() {
                match Utf8PathBuf::from_path_buf(path) {
                    Ok(path) => paths.push(path),
                    Err(e) => println!("{:?}", e),
                }
            }
        }
        Err(e) => println!("{:?}", e),
    }
    paths
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
    let a: Vec<_> = std::env::args().collect();
    println!("main args: {a:?}");
    let Cli {
        ev2,
        environments,
        scratch,
    } = &Cli::parse();

    let ev2_path = Utf8PathBuf::from_str(ev2)?;
    let environments_path = ev2_path.join(environments);
    let scratch_path = ev2_path.join(scratch);

    let flags = load_flags(&ev2_path)?;

    let environments_yml_paths = list_yml_paths(&environments_path);
    let yml_files = environments_yml_paths
        .iter()
        .map(|x| {
            x.strip_prefix(&environments_path)
                .with_context(|| "strip prefix")
        })
        .collect::<Result<Vec<_>>>()?;
    let dirs_files = group_yml_files_by_dir(yml_files);
    let dirs_files: BTreeMap<&Utf8Path, Vec<&Utf8Path>> = dirs_files
        .iter()
        .map(|(k, v)| (k.as_path(), v.clone()))
        .collect();

    let mut input_cache = HashMap::new();

    let dirs = dirs_files.keys();
    for dir in dirs {
        let dump_json_path = scratch_path.join(dir).join("dump2.json");
        println!("dump_json_path: {dump_json_path}");
        let mut dump_json = json!({});

        let mut input_yml_paths = Vec::new();
        let mut ancestors = dir.ancestors().into_iter().collect::<Vec<_>>();
        ancestors.reverse();

        // add flags
        for ancestor in &ancestors {
            let path = environments_path
                .join(ancestor)
                .strip_prefix(&ev2_path)?
                .to_string();
            if let Some(json) = flags.get(&path) {
                dump_json = dump_json.merged_recursive::<Dfs>(json);
            }
        }

        for ancestor in &ancestors {
            if let Some(dir_files) = dirs_files.get(ancestor) {
                for file in dir_files {
                    input_yml_paths.push(file);
                }
            }
        }

        // add environments
        for input_yml_path in input_yml_paths {
            if let Some(json) = input_cache.get(&input_yml_path) {
                dump_json = dump_json.merged_recursive::<Dfs>(json);
            } else {
                let path_full = environments_path.join(input_yml_path);
                let json: serde_json::Value = serde_yaml::from_slice(
                    &fs::read(&path_full).with_context(|| format!("reading file {path_full}"))?,
                )
                .with_context(|| format!("reading json {path_full}"))?;
                dump_json = dump_json.merged_recursive::<Dfs>(&json);
                input_cache.insert(input_yml_path, json);
            }
        }

        dump_json.sort_keys();
        let dir = dump_json_path
            .parent()
            .with_context(|| "parent of {dump_json_path}")?;
        if !dir.exists() {
            fs::create_dir_all(dir).with_context(|| format!("creating {dir}"))?;
        }
        fs::write(&dump_json_path, serde_json::to_string_pretty(&dump_json)?)
            .with_context(|| format!("writing {dump_json_path}"))?;
    }
    Ok(())
}

fn load_flags(ev2: &Utf8Path) -> Result<HashMap<String, Value>> {
    let mut flags = HashMap::new();
    let path = ev2.join("flags.yml");
    let json: serde_json::Value = serde_yaml::from_slice(&fs::read(&path)?)?;
    for (key, values) in json.as_object().unwrap() {
        for (value, paths) in values.as_object().unwrap() {
            for path in paths.as_array().unwrap() {
                let path = path.as_str().unwrap();
                if flags.contains_key(path) {
                    let pairs: &mut BTreeMap<String, String> = flags.get_mut(path).unwrap();
                    pairs.insert(key.to_string(), value.to_string());
                } else {
                    let mut pairs = BTreeMap::new();
                    pairs.insert(key.to_string(), value.to_string());
                    flags.insert(path.to_string(), pairs);
                }
            }
        }
    }
    // convert values to json
    let flags = flags
        .into_iter()
        .map(|(path, pairs)| {
            let mut map = serde_json::Map::new();
            pairs.into_iter().for_each(|(key, value)| {
                let value = match value.as_str() {
                    "true" => json!(true),
                    "false" => json!(false),
                    _ => json!(value),
                };
                map.insert(key, value);
            });
            (path, Value::Object(map))
        })
        .collect();
    Ok(flags)
}
