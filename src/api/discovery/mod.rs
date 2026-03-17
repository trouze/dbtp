pub mod exposures;
pub mod lineage;
pub mod macros;
pub mod models;
pub mod resource_details;
pub mod sources;

use serde_json::Value;

use crate::core::config::Config;
use crate::core::error::{DbtpError, Result};
use crate::core::graphql_client::GraphqlClient;

const DEFAULT_PAGE_SIZE: usize = 100;
const MAX_NODE_QUERY_LIMIT: usize = 10_000;

pub const DBT_BUILTIN_PACKAGES: &[&str] = &[
    "dbt",
    "dbt_postgres",
    "dbt_redshift",
    "dbt_snowflake",
    "dbt_bigquery",
    "dbt_spark",
    "dbt_athena",
];

/// Derive the Discovery metadata API base URL from the config host.
/// e.g. "https://cloud.getdbt.com" -> "https://metadata.cloud.getdbt.com"
pub fn metadata_url(host: &str) -> String {
    let host = host.trim_end_matches('/');
    if host.contains("metadata.") {
        return host.to_string();
    }
    if let Some(rest) = host.strip_prefix("https://") {
        format!("https://metadata.{rest}")
    } else if let Some(rest) = host.strip_prefix("http://") {
        format!("http://metadata.{rest}")
    } else {
        format!("https://metadata.{host}")
    }
}

pub fn require_environment_id(config: &Config) -> Result<u64> {
    config
        .environment_id_u64()
        .ok_or_else(|| {
            DbtpError::config(
                "environment_id is required for Discovery API; \
                 set via --environment-id, DBTP_ENVIRONMENT_ID, or `dbtp configure`",
            )
        })
}

/// Extract nodes from paginated GraphQL edges: `[{node: ...}, ...]` -> `[...]`
pub fn extract_nodes(edges: &Value) -> Vec<Value> {
    edges
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|edge| edge.get("node"))
                .filter(|node| node.is_object())
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// Navigate a JSON value by a sequence of keys.
pub fn extract_path<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    let mut current = value;
    for key in path {
        current = &current[key];
    }
    current
}

/// Cursor-based pagination for the Discovery GraphQL API.
///
/// Fetches pages of results using `after`/`first` variables,
/// collecting all nodes until `hasNextPage` is false or `MAX_NODE_QUERY_LIMIT` is reached.
pub async fn paginate(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    query: &str,
    base_variables: Value,
    edges_path: &[&str],
    page_info_path: &[&str],
) -> Result<Vec<Value>> {
    let meta_host = metadata_url(host);
    let mut collected: Vec<Value> = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        if collected.len() >= MAX_NODE_QUERY_LIMIT {
            break;
        }

        let remaining = MAX_NODE_QUERY_LIMIT - collected.len();
        let page_size = remaining.min(DEFAULT_PAGE_SIZE);

        let mut vars = base_variables.clone();
        if let Some(obj) = vars.as_object_mut() {
            obj.insert("environmentId".into(), environment_id.into());
            obj.insert("first".into(), (page_size as u64).into());
            if let Some(ref c) = cursor {
                obj.insert("after".into(), c.clone().into());
            }
        }

        let data = client
            .discovery(&meta_host, environment_id, query, Some(vars))
            .await?;

        let edges = extract_path(&data, edges_path);
        let nodes = extract_nodes(edges);
        collected.extend(nodes);

        let page_info = extract_path(&data, page_info_path);
        let has_next = page_info["hasNextPage"].as_bool().unwrap_or(false);
        let end_cursor = page_info["endCursor"].as_str().map(String::from);

        let should_continue = has_next
            && end_cursor
                .as_ref()
                .map_or(false, |c| cursor.as_ref().map_or(true, |prev| prev != c));

        if should_continue {
            cursor = end_cursor;
        } else {
            break;
        }
    }

    Ok(collected)
}

/// Determine if an identifier is a unique_id (contains dots) vs a plain name.
pub fn is_unique_id(identifier: &str) -> bool {
    identifier.contains('.')
}
