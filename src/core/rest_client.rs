use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::Value;

use super::error::{DbtpError, Result};

#[derive(Debug, Clone)]
pub struct RestClient {
    client: reqwest::Client,
    host: String,
    token: String,
    account_id: Option<u64>,
}

impl RestClient {
    pub fn new(host: &str, token: &str, account_id: Option<u64>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(DbtpError::Http)?;

        Ok(Self {
            client,
            host: host.trim_end_matches('/').to_string(),
            token: token.to_string(),
            account_id,
        })
    }

    pub fn account_id(&self) -> Result<u64> {
        self.account_id
            .ok_or_else(|| DbtpError::config("account_id is required; set via --account-id, DBTP_ACCOUNT_ID, or `dbtp init`"))
    }

    /// v2 base: /api/v2/accounts/{id}
    pub fn v2_url(&self, path: &str) -> Result<String> {
        let aid = self.account_id()?;
        Ok(format!("{}/api/v2/accounts/{aid}/{path}", self.host))
    }

    /// v3 base: /api/v3/accounts/{id}
    pub fn v3_url(&self, path: &str) -> Result<String> {
        let aid = self.account_id()?;
        Ok(format!("{}/api/v3/accounts/{aid}/{path}", self.host))
    }

    /// v2 uses `Token` auth, v3 uses `Bearer` auth.
    fn auth_header(&self, version: ApiVersion) -> HeaderValue {
        let val = match version {
            ApiVersion::V2 => format!("Token {}", self.token),
            ApiVersion::V3 => format!("Bearer {}", self.token),
        };
        HeaderValue::from_str(&val).unwrap_or_else(|_| HeaderValue::from_static(""))
    }

    pub async fn get_v2(&self, path: &str, params: &[(String, String)]) -> Result<Value> {
        let url = self.v2_url(path)?;
        self.get_raw(&url, params, ApiVersion::V2).await
    }

    pub async fn get_v3(&self, path: &str, params: &[(String, String)]) -> Result<Value> {
        let url = self.v3_url(path)?;
        self.get_raw(&url, params, ApiVersion::V3).await
    }

    pub async fn post_v2(&self, path: &str, body: &Value) -> Result<Value> {
        let url = self.v2_url(path)?;
        self.post_raw(&url, body, ApiVersion::V2).await
    }

    pub async fn post_v3(&self, path: &str, body: &Value) -> Result<Value> {
        let url = self.v3_url(path)?;
        self.post_raw(&url, body, ApiVersion::V3).await
    }

    pub async fn patch_v2(&self, path: &str, body: &Value) -> Result<Value> {
        let url = self.v2_url(path)?;
        self.patch_raw(&url, body, ApiVersion::V2).await
    }

    pub async fn patch_v3(&self, path: &str, body: &Value) -> Result<Value> {
        let url = self.v3_url(path)?;
        self.patch_raw(&url, body, ApiVersion::V3).await
    }

    pub async fn delete_v2(&self, path: &str) -> Result<Value> {
        let url = self.v2_url(path)?;
        self.delete_raw(&url, ApiVersion::V2).await
    }

    pub async fn delete_v3(&self, path: &str) -> Result<Value> {
        let url = self.v3_url(path)?;
        self.delete_raw(&url, ApiVersion::V3).await
    }

    async fn get_raw(
        &self,
        url: &str,
        params: &[(String, String)],
        version: ApiVersion,
    ) -> Result<Value> {
        let resp = self
            .client
            .get(url)
            .header(AUTHORIZATION, self.auth_header(version))
            .query(params)
            .send()
            .await
            .map_err(DbtpError::Http)?;

        self.handle_response(resp).await
    }

    async fn post_raw(&self, url: &str, body: &Value, version: ApiVersion) -> Result<Value> {
        let resp = self
            .client
            .post(url)
            .header(AUTHORIZATION, self.auth_header(version))
            .json(body)
            .send()
            .await
            .map_err(DbtpError::Http)?;

        self.handle_response(resp).await
    }

    async fn patch_raw(&self, url: &str, body: &Value, version: ApiVersion) -> Result<Value> {
        let resp = self
            .client
            .patch(url)
            .header(AUTHORIZATION, self.auth_header(version))
            .json(body)
            .send()
            .await
            .map_err(DbtpError::Http)?;

        self.handle_response(resp).await
    }

    async fn delete_raw(&self, url: &str, version: ApiVersion) -> Result<Value> {
        let resp = self
            .client
            .delete(url)
            .header(AUTHORIZATION, self.auth_header(version))
            .send()
            .await
            .map_err(DbtpError::Http)?;

        self.handle_response(resp).await
    }

    async fn handle_response(&self, resp: reqwest::Response) -> Result<Value> {
        let status = resp.status();
        let body: Value = resp.json().await.map_err(DbtpError::Http)?;

        if !status.is_success() {
            let message = body["status"]["user_message"]
                .as_str()
                .or_else(|| body["message"].as_str())
                .unwrap_or("Unknown error")
                .to_string();
            return Err(DbtpError::api(status.as_u16(), message));
        }

        Ok(unwrap_envelope(body))
    }

    /// Auto-paginate a v2 list endpoint, collecting all results.
    pub async fn paginate_v2(
        &self,
        path: &str,
        base_params: &[(String, String)],
        limit: Option<u64>,
    ) -> Result<Vec<Value>> {
        self.paginate_inner(path, base_params, limit, ApiVersion::V2)
            .await
    }

    /// Auto-paginate a v3 list endpoint, collecting all results.
    pub async fn paginate_v3(
        &self,
        path: &str,
        base_params: &[(String, String)],
        limit: Option<u64>,
    ) -> Result<Vec<Value>> {
        self.paginate_inner(path, base_params, limit, ApiVersion::V3)
            .await
    }

    async fn paginate_inner(
        &self,
        path: &str,
        base_params: &[(String, String)],
        limit: Option<u64>,
        version: ApiVersion,
    ) -> Result<Vec<Value>> {
        let page_size: u64 = 100;
        let mut offset: u64 = 0;
        let mut all_results: Vec<Value> = Vec::new();

        loop {
            let mut params = base_params.to_vec();
            params.push(("limit".into(), page_size.to_string()));
            params.push(("offset".into(), offset.to_string()));

            let url = match version {
                ApiVersion::V2 => self.v2_url(path)?,
                ApiVersion::V3 => self.v3_url(path)?,
            };

            let resp = self
                .client
                .get(&url)
                .header(AUTHORIZATION, self.auth_header(version))
                .query(&params)
                .send()
                .await
                .map_err(DbtpError::Http)?;

            let status = resp.status();
            let body: Value = resp.json().await.map_err(DbtpError::Http)?;

            if !status.is_success() {
                let message = body["status"]["user_message"]
                    .as_str()
                    .unwrap_or("Unknown error")
                    .to_string();
                return Err(DbtpError::api(status.as_u16(), message));
            }

            let data = &body["data"];
            if let Some(arr) = data.as_array() {
                all_results.extend(arr.iter().cloned());
            } else {
                all_results.push(data.clone());
                break;
            }

            if let Some(max) = limit {
                if all_results.len() as u64 >= max {
                    all_results.truncate(max as usize);
                    break;
                }
            }

            let total_count = body["extra"]["pagination"]["total_count"]
                .as_u64()
                .unwrap_or(0);
            offset += page_size;
            if offset >= total_count {
                break;
            }
        }

        Ok(all_results)
    }
}

#[derive(Debug, Clone, Copy)]
enum ApiVersion {
    V2,
    V3,
}

/// Strip the dbt Cloud envelope, returning `data` if present.
fn unwrap_envelope(body: Value) -> Value {
    if body.get("data").is_some() {
        body["data"].clone()
    } else {
        body
    }
}
