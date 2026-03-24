use serde_json::{json, Value};

use crate::core::error::Result;
use crate::core::graphql_client::GraphqlClient;

use super::paginate;

const GET_EXPOSURES: &str = include_str!("queries/get_exposures.graphql");

pub async fn list(client: &GraphqlClient, host: &str, environment_id: u64) -> Result<Vec<Value>> {
    paginate(
        client,
        host,
        environment_id,
        GET_EXPOSURES,
        json!({}),
        &["environment", "definition", "exposures", "edges"],
        &["environment", "definition", "exposures", "pageInfo"],
    )
    .await
}
