use anyhow::anyhow;
use dep_graph::{DepGraph, Node};
use ipnet::IpNet;
use minijinja::Error;
use minijinja::ErrorKind;
use minijinja::{Environment, Value};
use serde_json_merge::Dfs;
use serde_json_merge::Iter;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::str::FromStr;

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

    // build graph of variables
    pub fn graph(
        mut self,
        env: &Environment,
        value: &mut serde_json::Value,
    ) -> anyhow::Result<Graph> {
        let mut globals: HashMap<String, serde_json::Value> = HashMap::new();
        let mut invalid_templates = BTreeSet::new();

        value.iter::<Dfs>().for_each(|(path, value)| {
            if let Some(name) = path.last() {
                let name = name.to_string();
                globals.insert(name.clone(), value.clone());
                let vars = variables(env, value, &mut invalid_templates);
                for var in &vars {
                    self.get_or_create(var);
                    self.add_dep(&name, var);
                }
            }
        });
        if !invalid_templates.is_empty() {
            return Err(anyhow!("invalid templates: {invalid_templates:?}"));
        }

        let nodes = self.named_nodes.into_values().collect::<Vec<_>>();
        Ok(Graph {
            dep_graph: DepGraph::new(&nodes),
            globals,
        })
    }
}

struct Graph {
    dep_graph: DepGraph<String>,
    globals: HashMap<String, serde_json::Value>,
}

fn create_env<'s>() -> Environment<'s> {
    let mut env = Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
    env.add_filter("string", string);
    env.add_filter("int", int);
    env.add_filter("nthhost", nthhost);
    env.add_filter("ipaddr", ipaddr);
    env.add_filter("ipsubnet", ipsubnet);
    env
}

fn create_ctx() -> Value {
    Value::default()
}

// get undeclared variables from Value
fn variables(
    env: &Environment,
    value: &serde_json::Value,
    invalid_templates: &mut BTreeSet<String>,
) -> HashSet<String> {
    let mut vars = HashSet::new();
    value.iter_recursive::<Dfs>().for_each(|(_path, value)| {
        if let Some(value) = value.as_str() {
            if value.contains("{{") {
                if let Ok(tmpl) = env.template_from_str(value) {
                    vars.extend(tmpl.undeclared_variables(false));
                } else {
                    invalid_templates.insert(value.into());
                }
            }
        }
    });
    vars
}

/// Renders any values that are jinja templates.
/// The keys are set as global varaibles.
pub fn render(value: &mut serde_json::Value) -> anyhow::Result<()> {
    let mut env = create_env();
    let ctx = create_ctx();

    let var_nodes = VarNodes::default();
    let graph = var_nodes.graph(&env, value)?;
    graph.dep_graph.into_iter().for_each(|name| {
        if let Some(global) = graph.globals.get(&name) {
            if let Some(global) = global.as_str() {
                if let Ok(global) = env.render_str(global, &ctx) {
                    env.add_global(name.clone(), global);
                }
            }
            if let Some(global) = global.as_array() {
                let global = global
                    .iter()
                    .filter_map(|tmpl| {
                        if let Some(tmpl) = tmpl.as_str() {
                            env.render_str(tmpl, &ctx).ok()
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                env.add_global(name, global);
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

fn int(value: &str) -> Result<i32, Error> {
    value.parse::<i32>().map_err(|err| {
        Error::new(ErrorKind::InvalidOperation, "cannot convert to int").with_source(err)
    })
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

fn ipaddr(network: String, action: &Value) -> Result<String, Error> {
    if let Some(action) = action.as_str() {
        if let Ok(n) = action.parse::<usize>() {
            return nthhost(network, n);
        }
        if action == "address" {
            if let Ok(net) = IpNet::from_str(&network) {
                return Ok(net.network().to_string());
            }
        }
    }
    Ok(network)
}

fn ipsubnet(ip_network: String, prefix_len: u8, incr: usize) -> Result<String, Error> {
    let net = IpNet::from_str(&ip_network).map_err(|err| {
        Error::new(ErrorKind::InvalidOperation, format!("invalid IP network {ip_network}")).with_source(err)
    })?;
    let mut subnets = net.subnets(prefix_len).map_err(|err| {
        Error::new(ErrorKind::InvalidOperation, format!("cannot get subnets of {net} with prefix length {prefix_len}")).with_source(err)
    })?;
    let subnet = subnets.nth(incr).ok_or(Error::new(
        ErrorKind::InvalidOperation,
        format!("cannot get {incr}th subnet of {net} with prefix length {prefix_len}"),
    ))?;
    Ok(subnet.to_string())
}


#[cfg(test)]
pub mod test {
    use anyhow::ensure;
    use serde_json::json;
    use super::*;

    #[test]
    fn test_ipsubnet() -> anyhow::Result<()> {
        assert_render("{{ '100.73.148.0/22' | ipsubnet(24, 3) }}", "100.73.151.0/24")?;
        Ok(())
    }

    impl Graph {
        fn has_global(&self, name: &str) -> bool {
            self.globals.contains_key(name)
        }
        fn global(&self, name: &str) -> Option<&serde_json::Value> {
            self.globals.get(name)
        }
    }

    fn ensure_variables(value: &serde_json::Value, expected: &[&str]) -> anyhow::Result<()> {
        let mut invalid_templates = BTreeSet::new();
        let env = create_env();
        let actual = variables(&env, value, &mut invalid_templates);
        let expected = expected
            .iter()
            .map(|s| s.to_string())
            .collect::<HashSet<_>>();
        if actual != expected {
            return Err(anyhow!(
                "expected: {expected:?}, actual: {actual:?}, value: {value}"
            ));
        }
        Ok(())
    }

    #[test]
    fn test_variables() -> anyhow::Result<()> {
        ensure_variables(&json!("{{ a | replace('b', 'e') }}"), &["a"])?;
        ensure_variables(&json!("{{ a | replace(b, 'e') }}"), &["a", "b"])?;
        ensure_variables(&json!("{{ a | replace(b, e) }}"), &["a", "b", "e"])?;

        let value = serde_json::json!({
            "a": "bcd",
            "v": "{{ a | replace('b', 'e') }}",
            "colors": [
                "red",
                "green",
                "{{ blue | string }}"
            ],
        });
        ensure_variables(&value, &["a", "blue"])?;
        Ok(())
    }

    #[test]
    fn test_global() -> anyhow::Result<()> {
        let mut value = serde_json::json!({
            "a": "bcd",
            "v": "{{ a | replace('b', 'e') }}",
            "colors": [
                "red",
                "green",
                "blue"
            ],
        });
        let var_nodes = VarNodes::default();
        let env = create_env();
        let graph = var_nodes.graph(&env, &mut value)?;

        ensure!(graph.has_global("a"));
        ensure!(!graph.has_global("b"));
        ensure!(graph.has_global("v"));
        ensure!(graph.has_global("colors"));

        ensure!(graph.global("a") == Some(&json!("bcd")));
        ensure!(graph.global("b") == None);
        ensure!(graph.global("v") == Some(&json!("{{ a | replace('b', 'e') }}")));
        ensure!(graph.global("colors") == Some(&json!(["red", "green", "blue"])));

        let dep_graph = graph.dep_graph.into_iter().collect::<Vec<_>>();
        ensure!(dep_graph == vec!["a", "v"]);

        Ok(())
    }

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
    fn test_int() -> anyhow::Result<()> {
        assert_render("{{ '7' | int }}", "7")?;
        Ok(())
    }

    #[test]
    fn test_nthhost() -> anyhow::Result<()> {
        assert_render("{{ '10.0.0.0/8' | nthhost(0) }}", "10.0.0.0")?;
        assert_render("{{ '10.0.0.0/8' | nthhost(1) }}", "10.0.0.1")?;
        Ok(())
    }

    #[test]
    fn test_ipaddr() -> anyhow::Result<()> {
        assert_render("{{ '10.0.0.0/8' | ipaddr('0') }}", "10.0.0.0")?;
        assert_render("{{ '10.0.0.0/8' | ipaddr('1') }}", "10.0.0.1")?;
        assert_render("{{ '10.0.0.0/8' | ipaddr('address') }}", "10.0.0.0")?;
        assert_render("{{ '10.0.0.0' | ipaddr('address') }}", "10.0.0.0")?;
        assert_render(
            "{{ '169.254.0.0/30' | ipaddr('2') | ipaddr('address') }}",
            "169.254.0.2",
        )?;
        Ok(())
    }
}
