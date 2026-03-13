use serde_json::{json, Value};

use crate::core::error::Result;
use crate::core::graphql_client::GraphqlClient;

use super::{is_unique_id, metadata_url, paginate};

const GET_MODELS: &str = include_str!("queries/get_models.graphql");
const GET_MODEL_PARENTS: &str = include_str!("queries/get_model_parents.graphql");
const GET_MODEL_CHILDREN: &str = include_str!("queries/get_model_children.graphql");
const GET_MODEL_HEALTH: &str = include_str!("queries/get_model_health.graphql");
const GET_MODEL_PERFORMANCE: &str = include_str!("queries/get_model_performance.graphql");

fn model_filter(identifier: &str) -> Value {
    if is_unique_id(identifier) {
        json!({"uniqueIds": [identifier]})
    } else {
        json!({"identifier": identifier})
    }
}

pub async fn list(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    mart_only: bool,
) -> Result<Vec<Value>> {
    let filter = if mart_only {
        json!({"modelingLayer": "marts"})
    } else {
        json!({})
    };

    let nodes = paginate(
        client,
        host,
        environment_id,
        GET_MODELS,
        json!({
            "modelsFilter": filter,
            "sort": {"field": "queryUsageCount", "direction": "desc"},
        }),
        &["environment", "applied", "models", "edges"],
        &["environment", "applied", "models", "pageInfo"],
    )
    .await?;

    if mart_only {
        Ok(nodes
            .into_iter()
            .filter(|n| n["name"].as_str() != Some("metricflow_time_spine"))
            .collect())
    } else {
        Ok(nodes)
    }
}

pub async fn parents(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    identifier: &str,
) -> Result<Value> {
    let meta_host = metadata_url(host);
    let vars = json!({
        "environmentId": environment_id,
        "modelsFilter": model_filter(identifier),
        "first": 1,
    });

    let data = client
        .discovery(&meta_host, environment_id, GET_MODEL_PARENTS, Some(vars))
        .await?;

    let edges = &data["environment"]["applied"]["models"]["edges"];
    match edges.as_array().and_then(|arr| arr.first()) {
        Some(edge) => Ok(edge["node"]["parents"].clone()),
        None => Ok(Value::Array(vec![])),
    }
}

pub async fn children(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    identifier: &str,
) -> Result<Value> {
    let meta_host = metadata_url(host);
    let vars = json!({
        "environmentId": environment_id,
        "modelsFilter": model_filter(identifier),
        "first": 1,
    });

    let data = client
        .discovery(&meta_host, environment_id, GET_MODEL_CHILDREN, Some(vars))
        .await?;

    let edges = &data["environment"]["applied"]["models"]["edges"];
    match edges.as_array().and_then(|arr| arr.first()) {
        Some(edge) => Ok(edge["node"]["children"].clone()),
        None => Ok(Value::Array(vec![])),
    }
}

pub async fn health(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    identifier: &str,
) -> Result<Value> {
    let meta_host = metadata_url(host);
    let vars = json!({
        "environmentId": environment_id,
        "modelsFilter": model_filter(identifier),
        "first": 1,
    });

    let data = client
        .discovery(&meta_host, environment_id, GET_MODEL_HEALTH, Some(vars))
        .await?;

    let edges = &data["environment"]["applied"]["models"]["edges"];
    match edges.as_array().and_then(|arr| arr.first()) {
        Some(edge) => Ok(edge["node"].clone()),
        None => Ok(Value::Null),
    }
}

pub async fn performance(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    unique_id: &str,
    num_runs: u32,
    include_tests: bool,
) -> Result<Value> {
    let meta_host = metadata_url(host);
    let vars = json!({
        "environmentId": environment_id,
        "uniqueId": unique_id,
        "lastRunCount": num_runs,
    });

    let data = client
        .discovery(&meta_host, environment_id, GET_MODEL_PERFORMANCE, Some(vars))
        .await?;

    let runs = &data["environment"]["applied"]["modelHistoricalRuns"];
    let mut result = runs.clone();

    if !include_tests {
        if let Some(arr) = result.as_array_mut() {
            for run in arr.iter_mut() {
                if let Some(obj) = run.as_object_mut() {
                    obj.remove("tests");
                }
            }
        }
    }

    Ok(result)
}
