use std::collections::BTreeSet;
use std::collections::HashMap;

use anyhow::anyhow;
use dep_graph::{DepGraph, Node};
use ipnet::IpNet;
use minijinja::Error;
use minijinja::ErrorKind;
use minijinja::{Environment, Value};
use serde_json_merge::Dfs;
use serde_json_merge::Iter;
use std::str::FromStr;

// implement default
#[derive(Default)]
struct VarNodes {
    named_nodes: HashMap<String, Node<String>>,
}

impl VarNodes {
    /// get or create node with name
    fn get_or_create(&mut self, name: &str) -> &mut Node<String> {
        if !self.named_nodes.contains_key(name) {
            let node = Node::new(name.to_string());
            self.named_nodes.insert(name.to_string(), node);
        }
        self.named_nodes.get_mut(name).unwrap()
    }
    fn add_dep(&mut self, source: &str, target: &str) {
        let source_node = self.get_or_create(source);
        source_node.add_dep(target.to_string());
    }
    fn graph(self) -> DepGraph<String> {
        let nodes = self.named_nodes.into_values().collect::<Vec<_>>();
        DepGraph::new(&nodes)
    }
}

fn create_env<'s>() -> Environment<'s> {
    let mut env = Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
    env.add_filter("string", string);
    env.add_filter("nthhost", nthhost);
    env
}

fn create_ctx() -> Value {
    Value::default()
}

/// Renders any values that are jinja templates.
/// The keys are set as global varaibles.
pub fn render(value: &mut serde_json::Value) -> anyhow::Result<()> {
    let mut env = create_env();
    let ctx = create_ctx();

    let mut templates = HashMap::new();
    let mut invalid_templates = BTreeSet::new();

    // build graph of variables
    let mut var_nodes = VarNodes::default();
    value.iter::<Dfs>().for_each(|(path, value)| {
        if let Some(value) = value.as_str() {
            let name = path.last().unwrap().to_string();
            if value.contains("{{") {
                if let Ok(tmpl) = env.template_from_str(value) {
                    let vars = tmpl.undeclared_variables(false);
                    for var in &vars {
                        var_nodes.add_dep(&name, var);
                        var_nodes.get_or_create(var);
                    }
                    templates.insert(name.clone(), value);
                } else {
                    invalid_templates.insert(value);
                }
            } else {
                env.add_global(name, value);
            }
        }
    });
    if !invalid_templates.is_empty() {
        return Err(anyhow!("invalid templates: {invalid_templates:?}"));
    }

    let graph = var_nodes.graph();
    graph.into_iter().for_each(|name| {
        if let Some(tmpl) = templates.get(&name) {
            if let Ok(value) = env.render_str(tmpl, &ctx) {
                env.add_global(name, value);
            }
        };
    });

    let mut render_errors = Vec::new();
    value
        .mutate_recursive::<Dfs>()
        .for_each(|_path, value: &mut serde_json::Value| {
            if let Some(val) = value.as_str() {
                if val.contains("{{") {
                    match env.render_str(val, &ctx) {
                        Ok(val) => *value = val.into(),
                        Err(err) => {
                            render_errors.push(err);
                        }
                    }
                }
            }
        });

    if !render_errors.is_empty() {
        return Err(anyhow!("render errors: {render_errors:?}"));
    }
    Ok(())
}

fn string(value: &Value) -> Result<String, Error> {
    Ok(value.to_string())
}

/// The nth IP address in a IP network.
fn nthhost(network: String, n: usize) -> Result<String, Error> {
    let net = IpNet::from_str(&network).map_err(|err| {
        Error::new(ErrorKind::InvalidOperation, "cannot get nthhost").with_source(err)
    })?;
    let ip = if n == 0 {
        net.network()
    } else {
        net.hosts().nth(n - 1).ok_or(Error::new(
            ErrorKind::InvalidOperation,
            "cannot get nthhost",
        ))?
    };
    Ok(ip.to_string())
}

#[cfg(test)]
pub mod test {
    use super::*;

    fn assert_render(tmpl_str: &str, expected: &str) -> anyhow::Result<()> {
        let env = create_env();
        let ctx = create_ctx();
        let actual = env.render_str(tmpl_str, ctx)?;
        if actual != expected {
            return Err(anyhow!(
                "expected: {expected}, actual: {actual}, template: {tmpl_str}"
            ));
        }
        Ok(())
    }

    #[test]
    fn test_string() -> anyhow::Result<()> {
        assert_render("{{ 'abc' | string }}", "abc")?;
        assert_render("{{ false | string }}", "false")?;
        assert_render("{{ 1 | string }}", "1")?;
        Ok(())
    }

    #[test]
    fn test_nthhost() -> anyhow::Result<()> {
        assert_render("{{ '10.0.0.0/8' | nthhost(0) }}", "10.0.0.0")?;
        assert_render("{{ '10.0.0.0/8' | nthhost(1) }}", "10.0.0.1")?;
        Ok(())
    }
}