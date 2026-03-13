use serde_json::{json, Value};

use crate::core::error::Result;
use crate::core::graphql_client::GraphqlClient;

const GET_METRICS: &str = include_str!("queries/get_metrics.graphql");
const GET_DIMENSIONS: &str = include_str!("queries/get_dimensions.graphql");
const GET_ENTITIES: &str = include_str!("queries/get_entities.graphql");
const GET_MEASURES: &str = include_str!("queries/get_measures.graphql");
const GET_QUERYABLE_GRANULARITIES: &str =
    include_str!("queries/get_queryable_granularities.graphql");
const GET_METRICS_FOR_DIMENSIONS: &str =
    include_str!("queries/get_metrics_for_dimensions.graphql");

fn metric_inputs(names: &[String]) -> Vec<Value> {
    names.iter().map(|n| json!({"name": n})).collect()
}

fn groupby_inputs(names: &[String]) -> Vec<Value> {
    names.iter().map(|n| json!({"name": n})).collect()
}

pub async fn list_metrics(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    search: Option<&str>,
) -> Result<Value> {
    let vars = json!({
        "environmentId": environment_id,
        "search": search,
    });
    let data = client
        .semantic_layer(host, environment_id, GET_METRICS, Some(vars))
        .await?;
    Ok(data
        .get("metricsPaginated")
        .and_then(|v| v.get("items"))
        .cloned()
        .unwrap_or(Value::Array(vec![])))
}

pub async fn list_dimensions(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    metrics: &[String],
    search: Option<&str>,
) -> Result<Value> {
    let vars = json!({
        "environmentId": environment_id,
        "metrics": metric_inputs(metrics),
        "search": search,
    });
    let data = client
        .semantic_layer(host, environment_id, GET_DIMENSIONS, Some(vars))
        .await?;
    Ok(data
        .get("dimensionsPaginated")
        .and_then(|v| v.get("items"))
        .cloned()
        .unwrap_or(Value::Array(vec![])))
}

pub async fn list_entities(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    metrics: &[String],
    search: Option<&str>,
) -> Result<Value> {
    let vars = json!({
        "environmentId": environment_id,
        "metrics": metric_inputs(metrics),
        "search": search,
    });
    let data = client
        .semantic_layer(host, environment_id, GET_ENTITIES, Some(vars))
        .await?;
    Ok(data
        .get("entitiesPaginated")
        .and_then(|v| v.get("items"))
        .cloned()
        .unwrap_or(Value::Array(vec![])))
}

pub async fn list_measures(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    metrics: &[String],
) -> Result<Value> {
    let vars = json!({
        "environmentId": environment_id,
        "metrics": metric_inputs(metrics),
    });
    let data = client
        .semantic_layer(host, environment_id, GET_MEASURES, Some(vars))
        .await?;
    Ok(data
        .get("measures")
        .cloned()
        .unwrap_or(Value::Array(vec![])))
}

pub async fn list_queryable_granularities(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    metrics: &[String],
) -> Result<Value> {
    let vars = json!({
        "environmentId": environment_id,
        "metrics": metric_inputs(metrics),
    });
    let data = client
        .semantic_layer(host, environment_id, GET_QUERYABLE_GRANULARITIES, Some(vars))
        .await?;
    Ok(data
        .get("queryableGranularities")
        .cloned()
        .unwrap_or(Value::Array(vec![])))
}

pub async fn list_metrics_for_dimensions(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    dimensions: &[String],
) -> Result<Value> {
    let vars = json!({
        "environmentId": environment_id,
        "dimensions": groupby_inputs(dimensions),
    });
    let data = client
        .semantic_layer(host, environment_id, GET_METRICS_FOR_DIMENSIONS, Some(vars))
        .await?;
    Ok(data
        .get("metricsForDimensions")
        .cloned()
        .unwrap_or(Value::Array(vec![])))
}
