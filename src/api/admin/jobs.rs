use serde_json::{json, Value};

use crate::core::error::Result;
use crate::core::rest_client::RestClient;

pub async fn list(
    client: &RestClient,
    params: &[(String, String)],
    limit: Option<u64>,
) -> Result<Vec<Value>> {
    client.paginate_v2("jobs/", params, limit).await
}

pub async fn get(client: &RestClient, job_id: u64) -> Result<Value> {
    client.get_v2(&format!("jobs/{job_id}/"), &[]).await
}

pub async fn create(client: &RestClient, body: &Value) -> Result<Value> {
    client.post_v2("jobs/", body).await
}

pub async fn update(client: &RestClient, job_id: u64, body: &Value) -> Result<Value> {
    client
        .post_v2(&format!("jobs/{job_id}/"), body)
        .await
}

pub async fn delete(client: &RestClient, job_id: u64) -> Result<Value> {
    client.delete_v2(&format!("jobs/{job_id}/")).await
}

pub async fn trigger(client: &RestClient, job_id: u64, body: &Value) -> Result<Value> {
    client
        .post_v2(&format!("jobs/{job_id}/run/"), body)
        .await
}

pub async fn trigger_from_failure(client: &RestClient, job_id: u64) -> Result<Value> {
    client
        .post_v2(&format!("jobs/{job_id}/rerun/"), &json!({}))
        .await
}
