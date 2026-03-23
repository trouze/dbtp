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

    /// Execute a GraphQL query against the Discovery API (stable endpoint).
    pub async fn discovery(
        &self,
        host: &str,
        environment_id: u64,
        query: &str,
        variables: Option<Value>,
    ) -> Result<Value> {
        self.discovery_request(host, "graphql", query, variables).await
    }

    /// Execute a GraphQL query against the Discovery API beta endpoint.
    /// Column-level lineage queries require the beta schema at `/beta/graphql`.
    pub async fn discovery_beta(
        &self,
        host: &str,
        environment_id: u64,
        query: &str,
        variables: Option<Value>,
    ) -> Result<Value> {
        self.discovery_request(host, "beta/graphql", query, variables).await
    }

    async fn discovery_request(
        &self,
        host: &str,
        path: &str,
        query: &str,
        variables: Option<Value>,
    ) -> Result<Value> {
        let url = format!(
            "{}/{}",
            host.trim_end_matches('/'),
            path
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

        Self::check_graphql_response(status, &body)?;

        Ok(body.get("data").cloned().unwrap_or(body))
    }

    /// Execute a GraphQL query against the Semantic Layer API.
    pub async fn semantic_layer(
        &self,
        host: &str,
        _environment_id: u64,
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

        Self::check_graphql_response(status, &body)?;

        Ok(body.get("data").cloned().unwrap_or(body))
    }

    fn extract_error_message(body: &Value) -> Option<String> {
        if let Some(errors) = body.get("errors").and_then(|e| e.as_array()) {
            if !errors.is_empty() {
                let messages: Vec<&str> = errors
                    .iter()
                    .filter_map(|e| e["message"].as_str())
                    .collect();
                if !messages.is_empty() {
                    return Some(messages.join("; "));
                }
            }
        }
        if let Some(msg) = body["message"].as_str() {
            return Some(msg.to_string());
        }
        if let Some(msg) = body["error"].as_str() {
            return Some(msg.to_string());
        }
        if let Some(msg) = body["detail"].as_str() {
            return Some(msg.to_string());
        }
        None
    }

    fn check_graphql_response(status: reqwest::StatusCode, body: &Value) -> Result<()> {
        let error_msg = Self::extract_error_message(body);

        if !status.is_success() {
            return Err(DbtpError::api(
                status.as_u16(),
                error_msg.as_deref().unwrap_or("GraphQL request failed"),
            ));
        }

        if let Some(msg) = error_msg {
            return Err(DbtpError::graphql(msg));
        }

        Ok(())
    }
}

fn bearer(token: &str) -> HeaderValue {
    HeaderValue::from_str(&format!("Bearer {token}"))
        .unwrap_or_else(|_| HeaderValue::from_static(""))
}
