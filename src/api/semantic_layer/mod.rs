pub mod metrics;
pub mod queries;
pub mod types;

/// Construct the Semantic Layer API base URL from a dbt Cloud host.
///
/// Multi-tenant hosts (cloud.getdbt.com, emea.dbt.com, au.dbt.com)
/// use the pattern `https://semantic-layer.{host}`.
pub fn semantic_layer_url(host: &str) -> String {
    let host = host
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    format!("https://semantic-layer.{host}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_layer_url_plain_host() {
        assert_eq!(
            semantic_layer_url("cloud.getdbt.com"),
            "https://semantic-layer.cloud.getdbt.com"
        );
    }

    #[test]
    fn test_semantic_layer_url_with_scheme() {
        assert_eq!(
            semantic_layer_url("https://emea.dbt.com"),
            "https://semantic-layer.emea.dbt.com"
        );
    }

    #[test]
    fn test_semantic_layer_url_with_trailing_slash() {
        assert_eq!(
            semantic_layer_url("https://au.dbt.com/"),
            "https://semantic-layer.au.dbt.com"
        );
    }
}
