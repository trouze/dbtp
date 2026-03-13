use serde_json::{json, Value};

use crate::core::error::Result;
use crate::core::graphql_client::GraphqlClient;

use super::paginate;

const GET_SOURCES: &str = include_str!("queries/get_sources.graphql");

pub async fn list(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    source_name: Option<&str>,
) -> Result<Vec<Value>> {
    let mut filter = json!({});
    if let Some(name) = source_name {
        filter = json!({"sourceNames": [name]});
    }

    paginate(
        client,
        host,
        environment_id,
        GET_SOURCES,
        json!({"sourcesFilter": filter}),
        &["environment", "applied", "sources", "edges"],
        &["environment", "applied", "sources", "pageInfo"],
    )
    .await
}
