use reqwest::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};

use super::error::{DbtpError, Result};

#[derive(Debug, Clone)]
pub struct GraphqlClient {
    client: reqwest::Client,
    token: String,
}

impl GraphqlClient {
    pub fn new(token: &str) -> Result<Self> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(DbtpError::Http)?;

        Ok(Self {
            client,
            token: token.to_string(),
        })
    }

    /// Execute a GraphQL query against the Discovery API.
    pub async fn discovery(
        &self,
        host: &str,
        environment_id: u64,
        query: &str,
        variables: Option<Value>,
    ) -> Result<Value> {
        let url = format!(
            "{}/graphql",
            host.trim_end_matches('/')
        );

        let body = json!({
            "query": query,
            "variables": variables.unwrap_or(json!({})),
        });

        let resp = self
            .client
            .post(&url)
            .header(AUTHORIZATION, bearer(&self.token))
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .header("X-dbt-partner-source", "dbtp-cli")
            .json(&body)
            .send()
            .await
            .map_err(DbtpError::Http)?;

        let status = resp.status();
        let body: Value = resp.json().await.map_err(DbtpError::Http)?;

        if !status.is_success() {
            return Err(DbtpError::api(
                status.as_u16(),
                body["message"].as_str().unwrap_or("GraphQL request failed"),
            ));
        }

        if let Some(errors) = body.get("errors") {
            if let Some(arr) = errors.as_array() {
                if !arr.is_empty() {
                    let messages: Vec<&str> = arr
                        .iter()
                        .filter_map(|e| e["message"].as_str())
                        .collect();
                    return Err(DbtpError::graphql(messages.join("; ")));
                }
            }
        }

        Ok(body.get("data").cloned().unwrap_or(body))
    }

    /// Execute a GraphQL query against the Semantic Layer API.
    pub async fn semantic_layer(
        &self,
        host: &str,
        environment_id: u64,
        query: &str,
        variables: Option<Value>,
    ) -> Result<Value> {
        let url = format!(
            "{}/api/graphql",
            host.trim_end_matches('/')
        );

        let body = json!({
            "query": query,
            "variables": variables.unwrap_or(json!({})),
            "environmentId": environment_id,
        });

        let resp = self
            .client
            .post(&url)
            .header(AUTHORIZATION, bearer(&self.token))
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .json(&body)
            .send()
            .await
            .map_err(DbtpError::Http)?;

        let status = resp.status();
        let body: Value = resp.json().await.map_err(DbtpError::Http)?;

        if !status.is_success() {
            return Err(DbtpError::api(
                status.as_u16(),
                body["message"]
                    .as_str()
                    .unwrap_or("Semantic Layer request failed"),
            ));
        }

        if let Some(errors) = body.get("errors") {
            if let Some(arr) = errors.as_array() {
                if !arr.is_empty() {
                    let messages: Vec<&str> = arr
                        .iter()
                        .filter_map(|e| e["message"].as_str())
                        .collect();
                    return Err(DbtpError::graphql(messages.join("; ")));
                }
            }
        }

        Ok(body.get("data").cloned().unwrap_or(body))
    }
}

fn bearer(token: &str) -> HeaderValue {
    HeaderValue::from_str(&format!("Bearer {token}"))
        .unwrap_or_else(|_| HeaderValue::from_static(""))
}
