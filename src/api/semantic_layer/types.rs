// These types model Semantic Layer API response shapes for serde.
// Not all are directly constructed today; defined for schema documentation.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub description: Option<String>,
    pub r#type: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedQuery {
    pub name: String,
    pub description: Option<String>,
    pub label: Option<String>,
    #[serde(rename = "queryParams")]
    pub query_params: Option<SavedQueryParams>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedQueryParams {
    pub metrics: Option<Vec<MetricInput>>,
    #[serde(rename = "groupBy")]
    pub group_by: Option<Vec<GroupByInput>>,
    pub r#where: Option<Vec<WhereClause>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhereClause {
    #[serde(rename = "whereSqlTemplate")]
    pub where_sql_template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricInput {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupByInput {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grain: Option<String>,
    #[serde(rename = "datePart", skip_serializing_if = "Option::is_none")]
    pub date_part: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderByInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric: Option<MetricInput>,
    #[serde(rename = "groupBy", skip_serializing_if = "Option::is_none")]
    pub group_by: Option<GroupByInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descending: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhereInput {
    pub sql: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimension {
    pub name: String,
    pub description: Option<String>,
    pub r#type: Option<String>,
    pub label: Option<String>,
    #[serde(rename = "queryableGranularities")]
    pub queryable_granularities: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub name: String,
    pub description: Option<String>,
    pub r#type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measure {
    pub name: String,
    pub agg: Option<String>,
    #[serde(rename = "aggTimeDimension")]
    pub agg_time_dimension: Option<String>,
    pub expr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPage {
    #[serde(rename = "arrowResult")]
    pub arrow_result: Option<String>,
    pub error: Option<String>,
    #[serde(rename = "queryId")]
    pub query_id: Option<String>,
    pub sql: Option<String>,
    pub status: String,
    #[serde(rename = "totalPages")]
    pub total_pages: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_input_serialize() {
        let input = MetricInput {
            name: "revenue".to_string(),
        };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json, serde_json::json!({"name": "revenue"}));
    }

    #[test]
    fn test_groupby_input_serialize_with_grain() {
        let input = GroupByInput {
            name: "metric_time".to_string(),
            grain: Some("MONTH".to_string()),
            date_part: None,
        };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"name": "metric_time", "grain": "MONTH"})
        );
    }

    #[test]
    fn test_groupby_input_serialize_without_grain() {
        let input = GroupByInput {
            name: "region".to_string(),
            grain: None,
            date_part: None,
        };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json, serde_json::json!({"name": "region"}));
    }

    #[test]
    fn test_orderby_input_metric() {
        let input = OrderByInput {
            metric: Some(MetricInput {
                name: "revenue".to_string(),
            }),
            group_by: None,
            descending: Some(true),
        };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"metric": {"name": "revenue"}, "descending": true})
        );
    }

    #[test]
    fn test_orderby_input_groupby() {
        let input = OrderByInput {
            metric: None,
            group_by: Some(GroupByInput {
                name: "metric_time".to_string(),
                grain: Some("DAY".to_string()),
                date_part: None,
            }),
            descending: Some(false),
        };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"groupBy": {"name": "metric_time", "grain": "DAY"}, "descending": false})
        );
    }

    #[test]
    fn test_where_input_serialize() {
        let input = WhereInput {
            sql: "order_date > '2024-01-01'".to_string(),
        };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"sql": "order_date > '2024-01-01'"})
        );
    }

    #[test]
    fn test_query_page_deserialize() {
        let data = serde_json::json!({
            "arrowResult": "QUFB",
            "error": null,
            "queryId": "abc-123",
            "sql": "SELECT 1",
            "status": "SUCCESSFUL",
            "totalPages": 1
        });
        let page: QueryPage = serde_json::from_value(data).unwrap();
        assert_eq!(page.status, "SUCCESSFUL");
        assert_eq!(page.arrow_result.as_deref(), Some("QUFB"));
        assert_eq!(page.sql.as_deref(), Some("SELECT 1"));
        assert_eq!(page.total_pages, Some(1));
    }
}
