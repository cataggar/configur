use anyhow::Result;
use minijinja::{Environment, Value};

fn nthhost_filter(value: String, n: i32) -> String {
    println!("nthhost_filter {value} {n}");
    format!("{value}{n}")
}

#[cfg(test)]
pub mod test {
    use super::*;
    use minijinja::context;
    use std::collections::BTreeMap;

    #[test]
    fn test_remove_brackets() -> Result<()> {
        // let expr_str = "aks_static_services_subnet | string | nthhost(7)";
        // let expr_str = "'aks static services subnet' | string | nthhost(7)";
        // let expr_str = "1 + 4";

        let tmpl_str = "{{aks_static_services_subnet | nthhost(7)}}";

        BTreeMap::from_iter(vec![(1, 2), (3, 4)]);

        let mut variables = BTreeMap::new();
        variables.insert("aks_static_services_subnet", "100.73.5.0/24");
        let ctx = Value::from(variables);

        let mut env = Environment::new();
        // env.add_filter("string", string_filter);
        env.add_filter("nthhost", nthhost_filter);
        env.set_debug(true);

        // let expr = env.compile_expression(expr_str)?;
        // let result = expr.eval()?;

        let tmpl = env.template_from_str(tmpl_str)?;
        let (rv, _state) = tmpl.render_and_return_state(ctx)?;

        let serialized = serde_json::to_string_pretty(&rv)?;
        println!("serialized {serialized}");

        Ok(())
    }
}
