pub mod metrics;
pub mod queries;
pub mod types;

/// Construct the Semantic Layer API base URL from a dbt Cloud host.
///
/// Multi-tenant: `https://tk626.us1.dbt.com` → `https://tk626.semantic-layer.us1.dbt.com`
/// Legacy:       `https://cloud.getdbt.com`   → `https://semantic-layer.cloud.getdbt.com`
pub fn semantic_layer_url(host: &str) -> String {
    let host = host.trim_end_matches('/');

    let (scheme, rest) = if let Some(rest) = host.strip_prefix("https://") {
        ("https://", rest)
    } else if let Some(rest) = host.strip_prefix("http://") {
        ("http://", rest)
    } else {
        ("https://", host)
    };

    // Multi-tenant pattern: {slug}.{region}.dbt.com → {slug}.semantic-layer.{region}.dbt.com
    // Only applies when host has the form {slug}.{region}.dbt.com (two+ segments before .dbt.com)
    if let Some(prefix) = rest.strip_suffix(".dbt.com") {
        if prefix.contains('.') {
            if let Some(dot) = rest.find('.') {
                let slug = &rest[..dot];
                let remainder = &rest[dot + 1..];
                return format!("{scheme}{slug}.semantic-layer.{remainder}");
            }
        }
    }

    // Legacy: cloud.getdbt.com → semantic-layer.cloud.getdbt.com
    format!("{scheme}semantic-layer.{rest}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_layer_url_multi_tenant() {
        assert_eq!(
            semantic_layer_url("https://tk626.us1.dbt.com"),
            "https://tk626.semantic-layer.us1.dbt.com"
        );
    }

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
            semantic_layer_url("https://tk626.us1.dbt.com/"),
            "https://tk626.semantic-layer.us1.dbt.com"
        );
    }
}
