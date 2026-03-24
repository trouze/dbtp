use serde_json::{json, Value};

use crate::core::error::Result;
use crate::core::rest_client::RestClient;

pub async fn list(
    client: &RestClient,
    params: &[(String, String)],
    limit: Option<u64>,
) -> Result<Vec<Value>> {
    client.paginate_v2("runs/", params, limit).await
}

pub async fn get(client: &RestClient, run_id: u64) -> Result<Value> {
    client.get_v2(&format!("runs/{run_id}/"), &[]).await
}

pub async fn get_with_steps(client: &RestClient, run_id: u64) -> Result<Value> {
    let params = vec![("include_related".into(), "run_steps".into())];
    client.get_v2(&format!("runs/{run_id}/"), &params).await
}

pub async fn cancel(client: &RestClient, run_id: u64) -> Result<Value> {
    client
        .post_v2(&format!("runs/{run_id}/cancel/"), &json!({}))
        .await
}

pub async fn retry(client: &RestClient, run_id: u64) -> Result<Value> {
    client
        .post_v2(&format!("runs/{run_id}/retry/"), &json!({}))
        .await
}
