use std::collections::HashMap;

use anyhow::Result;
use minijinja::{Environment, Value};
use serde_json_merge::Dfs;
use serde_json_merge::Iter;
use dep_graph::{DepGraph, Node};

// implement default
#[derive(Default)]
struct VarNodes {
    named_nodes: HashMap<String, Node<String>>,
}

impl VarNodes {
    /// get or create node with name
    fn get_or_create(&mut self, name: &str) -> &mut Node<String> {
        if !self.named_nodes.contains_key((name)){
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

/// Renders any values that are jinja templates.
/// The keys are set as global varaibles.
pub fn render(value: &mut serde_json::Value) -> Result<()> {

    let mut env = Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);

    // let mut template_strings = Vec::new();
    

    // build graph of variables
    // let mut var_node: HashMap<String, Node<String>> = HashMap::new();
    // let mut variables: Vec<Node<String>> = Vec::new();
    let mut var_nodes = VarNodes::default();
    value.iter_recursive::<Dfs>().for_each(|(path, value)| {
        if let Some(value) = value.as_str() {

            // all root keys are variables
            // if let Some(root_key) = root_node {
            //     variables.push(variable);
            // }
            // if the value is a template
            if value.contains("{{") {
                let root_var: Option<String> = 
                if path.depth() == 1 {
                    Some(path.last().unwrap().to_string())
                } else {
                    None
                };
                if let Ok(tmpl) = env.template_from_str(value) {
                    let vars = tmpl.undeclared_variables(false);
                    for var in &vars {
                        if let Some(root_var) = &root_var {
                            var_nodes.add_dep(root_var, var)
                        }
                        var_nodes.get_or_create(var);
                    }
                }
            }
        }
    });

    // let graph = DepGraph::new(&variables);
    let graph = var_nodes.graph();
    graph
        .into_iter()
        .for_each(|node| {
            println!("{:?}", node)
        });

    // value
    //     .mutate_recursive::<Dfs>()
    //     .for_each(|_, val: &mut Value| {
    //         if let Some(obj) = val.as_object_mut() {
    //             if let Some(removed) = obj.remove("<<") {
    //                 val.merge_recursive::<Dfs>(&removed);
    //             }
    //         }
    //     });
    Ok(())
}

fn nthhost_filter(value: String, n: i32) -> String {
    println!("nthhost_filter {value} {n}");
    format!("{value}{n}")
}

#[cfg(test)]
pub mod test {
    use super::*;
    use minijinja::{context, UndefinedBehavior};
    use std::collections::BTreeMap;

    #[test]
    fn test_remove_brackets() -> Result<()> {
        // let expr_str = "aks_static_services_subnet | string | nthhost(7)";
        // let expr_str = "'aks static services subnet' | string | nthhost(7)";
        // let expr_str = "1 + 4";

        let tmpl_str = "{{aks_static_services_subnet | nthhost(7)}}";

        BTreeMap::from_iter(vec![(1, 2), (3, 4)]);

        // let mut variables = BTreeMap::new();
        // variables.insert("aks_static_services_subnet", "100.73.5.0/24");
        // let ctx = Value::from(variables);

        let mut env = Environment::new();
        // env.add_filter("string", string_filter);
        env.add_filter("nthhost", nthhost_filter);
        env.set_debug(true);
        env.set_undefined_behavior(UndefinedBehavior::Strict);
        env.add_global("aks_static_services_subnet", "100.73.5.0/24");
        env.add_global("host", "abc");

        // let expr = env.compile_expression(expr_str)?;
        // let result = expr.eval()?;

        // let tmpl = env.template_from_str(tmpl_str)?;

        let _tmpl_host = env.template_from_named_str("host", "{{aks_static_services_subnet | nthhost(7)}}")?;

        

        let tmpl_host2 = env.template_from_named_str("host2", "{{ host | nthhost(7)}}")?;
        let variables = tmpl_host2.undeclared_variables(false);
        println!("variables {variables:?}");
        

        // let (rv, _state) = tmpl_host2.render_and_return_state(ctx)?;
        let ctx = Value::default();
        let rv = tmpl_host2.render(ctx)?;
        println!("rv {rv}");

        let serialized = serde_json::to_string_pretty(&rv)?;
        println!("serialized {serialized}");

        Ok(())
    }
}
