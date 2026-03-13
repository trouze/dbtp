use serde_json::Value;

use crate::core::error::{DbtpError, Result};
use crate::core::rest_client::RestClient;

/// GET /api/v2/accounts/ — requires a URL without account_id in the path,
/// so we bypass RestClient and make a direct request.
async fn raw_v2_get(host: &str, token: &str, path: &str) -> Result<Value> {
    let url = format!("{}/api/v2/{path}", host.trim_end_matches('/'));
    let resp = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Token {token}"))
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(DbtpError::Http)?;

    let status = resp.status();
    let body: Value = resp.json().await.map_err(DbtpError::Http)?;

    if !status.is_success() {
        let msg = body["status"]["user_message"]
            .as_str()
            .unwrap_or("Unknown error");
        return Err(DbtpError::api(status.as_u16(), msg));
    }

    Ok(body.get("data").cloned().unwrap_or(body))
}

pub async fn list(host: &str, token: &str) -> Result<Value> {
    raw_v2_get(host, token, "accounts/").await
}

pub async fn get(client: &RestClient) -> Result<Value> {
    client.get_v2("", &[]).await
}

pub async fn get_by_id(host: &str, token: &str, account_id: u64) -> Result<Value> {
    raw_v2_get(host, token, &format!("accounts/{account_id}/")).await
}
