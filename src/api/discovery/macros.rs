use serde_json::{json, Value};

use crate::core::error::Result;
use crate::core::graphql_client::GraphqlClient;

use super::{paginate, DBT_BUILTIN_PACKAGES};

const GET_MACROS: &str = include_str!("queries/get_macros.graphql");

pub async fn list(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    package: Option<&str>,
    packages_only: bool,
    include_dbt_packages: bool,
) -> Result<Value> {
    let filter = json!({"types": ["Macro"]});

    let mut macros = paginate(
        client,
        host,
        environment_id,
        GET_MACROS,
        json!({"filter": filter}),
        &["environment", "applied", "resources", "edges"],
        &["environment", "applied", "resources", "pageInfo"],
    )
    .await?;

    if !include_dbt_packages {
        macros.retain(|m| {
            m["packageName"]
                .as_str()
                .is_none_or(|pkg| !is_builtin_package(pkg))
        });
    }

    if let Some(pkg) = package {
        let pkg_lower = pkg.to_lowercase();
        macros.retain(|m| {
            m["packageName"]
                .as_str()
                .is_some_and(|p| p.to_lowercase() == pkg_lower)
        });
    }

    if packages_only {
        let mut packages: Vec<String> = macros
            .iter()
            .filter_map(|m| m["packageName"].as_str().map(String::from))
            .collect();
        packages.sort();
        packages.dedup();
        return Ok(Value::Array(
            packages.into_iter().map(Value::String).collect(),
        ));
    }

    Ok(Value::Array(macros))
}

fn is_builtin_package(name: &str) -> bool {
    DBT_BUILTIN_PACKAGES
        .iter()
        .any(|&pkg| pkg.eq_ignore_ascii_case(name))
}
