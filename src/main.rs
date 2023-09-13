use anyhow::Context;
use anyhow::Result;
use camino::Utf8Path;
use camino::Utf8PathBuf;
use clap::Parser;
use glob::glob;
use serde_json::json;
use serde_json_merge::Dfs;
use serde_json_merge::Iter;
use serde_json_merge::Merge;
use serde_json_merge::SortKeys;
use std::collections::HashMap;
use std::{collections::BTreeMap, fs, str::FromStr};

mod jinga;

type JsonCache = HashMap<Utf8PathBuf, serde_json::Value>;

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[arg(long)]
    ev2: String,
    #[arg(short, long, default_value = "environments")]
    environments: String,
    #[arg(short, long, default_value = "scratch")]
    scratch: String,
    #[arg(short, long)]
    verbose: bool,
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
    let Cli {
        ev2,
        environments,
        scratch,
        verbose,
    } = &Cli::parse();

    let ev2_path = Utf8PathBuf::from_str(ev2)?;
    let environments_path = ev2_path.join(environments);
    let scratch_path = ev2_path.join(scratch);

    let flags = load_flags(&ev2_path.join("flags.yml"))?;
    let versions = load_flags(&ev2_path.join("versions.yml"))?;
    let includes = load_includes(&ev2_path)?;

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

    let mut json_cache = JsonCache::new();

    let dirs = dirs_files.keys();
    for dir in dirs {
        let dump_json_path = scratch_path.join(dir).join("dump2.json");
        println!("dump_json_path: {dump_json_path}");
        let mut dump_json = json!({});

        let mut ancestors = dir.ancestors().into_iter().collect::<Vec<_>>();
        ancestors.reverse();
        let ancestor_paths = ancestors
            .iter()
            .map(|ancestor| {
                environments_path
                    .join(ancestor)
                    .strip_prefix(&ev2_path)
                    .unwrap()
                    .to_string()
                    .replace('\\', "/")
            })
            .collect::<Vec<_>>();

        // add includes
        for path in &ancestor_paths {
            if let Some(yml_paths) = includes.get(path) {
                for yml_path in yml_paths {
                    dump_json = merge_yml(dump_json, &mut json_cache, yml_path)?;
                }
            }
        }

        // add flags & versions
        for path in &ancestor_paths {
            if let Some(json) = flags.get(path) {
                dump_json = dump_json.merged_recursive::<Dfs>(json);
            }
            if let Some(json) = versions.get(path) {
                dump_json = dump_json.merged_recursive::<Dfs>(json);
            }
        }

        // add environments
        let mut environment_yml_paths = Vec::new();
        for ancestor in &ancestors {
            if let Some(dir_files) = dirs_files.get(ancestor) {
                for file in dir_files {
                    environment_yml_paths.push(file);
                }
            }
        }
        for yml_path in environment_yml_paths {
            let yml_path = environments_path.join(yml_path);
            dump_json = merge_yml(dump_json, &mut json_cache, &yml_path)?;
        }

        dump_json.sort_keys_recursive::<Dfs>();

        match jinga::render(&mut dump_json) {
            Ok(_) => {}
            Err(err) => {
                if *verbose {
                    println!("render error: {err}");
                }
            }
        }

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

fn merge_yml(dump_json: serde_json::Value, json_cache: &mut JsonCache, yml_path: &Utf8Path) -> Result<serde_json::Value> {
    Ok(if let Some(json) = json_cache.get(yml_path) {
        dump_json.merged_recursive::<Dfs>(json)
    } else {
        let mut json: serde_json::Value = serde_yaml::from_slice(
            &fs::read(yml_path).with_context(|| format!("reading file {yml_path}"))?,
        )
        .with_context(|| format!("reading yml {yml_path}"))?;
        remove_brackets(&mut json)?;
        let value = dump_json.merged_recursive::<Dfs>(&json);
        json_cache.insert(yml_path.to_path_buf(), json);
        value
    })
}

fn load_flags(yml: &Utf8Path) -> Result<HashMap<String, serde_json::Value>> {
    let mut flags = HashMap::new();
    let mut json: serde_json::Value = serde_yaml::from_slice(&fs::read(yml)?)?;
    remove_brackets(&mut json)?;
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
            (path, serde_json::Value::Object(map))
        })
        .collect();
    Ok(flags)
}

fn load_includes(ev2_path: &Utf8Path) -> Result<HashMap<String, Vec<Utf8PathBuf>>> {
    let include_yml = ev2_path.join("include.yml");
    let mut json: serde_json::Value = serde_yaml::from_slice(
        &fs::read(&include_yml).with_context(|| format!("reading file {include_yml}"))?,
    )?;
    remove_brackets(&mut json)?;
    let mut include_paths = HashMap::new();
    for (key, values) in json.as_object().unwrap() {
        let values = values
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>();
        include_paths.insert(key, values);
    }

    let mut paths_cache: HashMap<Utf8PathBuf, Vec<Utf8PathBuf>> = HashMap::new();
    let mut includes = HashMap::new();
    for (key, values) in include_paths {
        let mut combined_paths = Vec::new();
        for value in values {
            let include_path = ev2_path.join(value);
            if let Some(paths) = paths_cache.get(&include_path) {
                combined_paths.extend(paths.clone());
            } else {
                let paths = list_yml_paths(&include_path);
                combined_paths.extend(paths.clone());
                paths_cache.insert(include_path, paths);
            }
        }
        includes.insert(key.to_string(), combined_paths);
    }
    Ok(includes)
}

fn remove_brackets(value: &mut serde_json::Value) -> Result<()> {
    value
        .mutate_recursive::<Dfs>()
        .for_each(|_, val: &mut serde_json::Value| {
            if let Some(obj) = val.as_object_mut() {
                if let Some(removed) = obj.remove("<<") {
                    val.merge_recursive::<Dfs>(&removed);
                }
            }
        });
    Ok(())
}

#[cfg(test)]
pub mod test {
    use super::*;

    #[test]
    fn test_remove_brackets() -> Result<()> {
        let mut value = json!({"Processors": [
          {
            "6140": {
              "<<": {
                "Model": "Intel(R) Xeon(R) Gold 6140 CPU @ 2.30GHz"
              },
              "CPUCount": 2,
              "TotalCores": 18
            }
          }
        ]});
        let expected = json!({"Processors": [
          {
            "6140": {
              "Model": "Intel(R) Xeon(R) Gold 6140 CPU @ 2.30GHz",
              "CPUCount": 2,
              "TotalCores": 18
            }
          }
        ]});
        remove_brackets(&mut value)?;
        assert_eq!(&value, &expected);
        Ok(())
    }
}
