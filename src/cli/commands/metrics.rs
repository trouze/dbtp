use clap::{Args, Subcommand};
use serde_json::Value;

use crate::api::semantic_layer;
use crate::api::semantic_layer::types::*;
use crate::core::config::Config;
use crate::core::error::{DbtpError, Result};
use crate::core::graphql_client::GraphqlClient;

#[derive(Debug, Args)]
pub struct MetricsArgs {
    #[command(subcommand)]
    pub command: MetricsCommand,
}

#[derive(Debug, Subcommand)]
pub enum MetricsCommand {
    /// List all metrics
    List {
        /// Search filter
        #[arg(long)]
        search: Option<String>,
    },
    /// List dimensions for given metrics
    Dimensions {
        /// Metric names
        metrics: Vec<String>,
        /// Search filter
        #[arg(long)]
        search: Option<String>,
    },
    /// List entities for given metrics
    Entities {
        /// Metric names
        metrics: Vec<String>,
        /// Search filter
        #[arg(long)]
        search: Option<String>,
    },
    /// List measures for given metrics
    Measures {
        /// Metric names
        metrics: Vec<String>,
    },
    /// List queryable granularities for given metrics
    Granularities {
        /// Metric names
        metrics: Vec<String>,
    },
    /// List metrics available for given dimensions
    ForDimensions {
        /// Dimension names
        dimensions: Vec<String>,
    },
    /// Execute a metric query and return results
    Query {
        /// Metric names
        metrics: Vec<String>,
        /// Group-by dimensions (comma-separated or repeated)
        #[arg(long, value_delimiter = ',')]
        group_by: Vec<String>,
        /// SQL WHERE filters (repeat for multiple)
        #[arg(long = "where")]
        where_: Vec<String>,
        /// Order-by fields (prefix with - for descending)
        #[arg(long, value_delimiter = ',')]
        order_by: Vec<String>,
        /// Maximum number of rows
        #[arg(long)]
        limit: Option<i64>,
        /// Time grain for metric_time (DAY, WEEK, MONTH, QUARTER, YEAR)
        #[arg(long)]
        grain: Option<String>,
    },
    /// Compile metric query to SQL without executing
    Sql {
        /// Metric names
        metrics: Vec<String>,
        /// Group-by dimensions (comma-separated or repeated)
        #[arg(long, value_delimiter = ',')]
        group_by: Vec<String>,
        /// SQL WHERE filters (repeat for multiple)
        #[arg(long = "where")]
        where_: Vec<String>,
        /// Order-by fields (prefix with - for descending)
        #[arg(long, value_delimiter = ',')]
        order_by: Vec<String>,
        /// Maximum number of rows
        #[arg(long)]
        limit: Option<i64>,
        /// Time grain for metric_time (DAY, WEEK, MONTH, QUARTER, YEAR)
        #[arg(long)]
        grain: Option<String>,
    },
}

fn require_env_id(config: &Config) -> Result<u64> {
    config.environment_id.ok_or_else(|| {
        DbtpError::config(
            "environment_id is required for Semantic Layer commands. \
             Set via --environment-id, DBTP_ENVIRONMENT_ID, or config profile.",
        )
    })
}

fn build_group_by(names: &[String], grain: &Option<String>) -> Vec<GroupByInput> {
    names
        .iter()
        .map(|name| {
            let grain = if name == "metric_time" {
                grain.as_ref().map(|g| g.to_uppercase())
            } else {
                None
            };
            GroupByInput {
                name: name.clone(),
                grain,
                date_part: None,
            }
        })
        .collect()
}

fn build_order_by(
    order_by: &[String],
    metrics: &[String],
    group_by_inputs: &[GroupByInput],
) -> Vec<OrderByInput> {
    order_by
        .iter()
        .map(|s| {
            let (name, desc) = if let Some(stripped) = s.strip_prefix('-') {
                (stripped, true)
            } else {
                (s.as_str(), false)
            };

            let is_metric = metrics.iter().any(|m| m == name);
            if is_metric {
                OrderByInput {
                    metric: Some(MetricInput {
                        name: name.to_string(),
                    }),
                    group_by: None,
                    descending: Some(desc),
                }
            } else {
                let gb = group_by_inputs.iter().find(|g| g.name == name);
                OrderByInput {
                    metric: None,
                    group_by: Some(gb.cloned().unwrap_or(GroupByInput {
                        name: name.to_string(),
                        grain: None,
                        date_part: None,
                    })),
                    descending: Some(desc),
                }
            }
        })
        .collect()
}

fn build_where(where_clauses: &[String]) -> Vec<WhereInput> {
    where_clauses
        .iter()
        .map(|sql| WhereInput { sql: sql.clone() })
        .collect()
}

pub async fn exec(
    args: &MetricsArgs,
    client: &GraphqlClient,
    config: &Config,
) -> Result<Value> {
    let env_id = require_env_id(config)?;
    let host = semantic_layer::semantic_layer_url(&config.host);

    match &args.command {
        MetricsCommand::List { search } => {
            semantic_layer::metrics::list_metrics(client, &host, env_id, search.as_deref()).await
        }
        MetricsCommand::Dimensions { metrics, search } => {
            semantic_layer::metrics::list_dimensions(
                client,
                &host,
                env_id,
                metrics,
                search.as_deref(),
            )
            .await
        }
        MetricsCommand::Entities { metrics, search } => {
            semantic_layer::metrics::list_entities(
                client,
                &host,
                env_id,
                metrics,
                search.as_deref(),
            )
            .await
        }
        MetricsCommand::Measures { metrics } => {
            semantic_layer::metrics::list_measures(client, &host, env_id, metrics).await
        }
        MetricsCommand::Granularities { metrics } => {
            semantic_layer::metrics::list_queryable_granularities(client, &host, env_id, metrics)
                .await
        }
        MetricsCommand::ForDimensions { dimensions } => {
            semantic_layer::metrics::list_metrics_for_dimensions(
                client, &host, env_id, dimensions,
            )
            .await
        }
        MetricsCommand::Query {
            metrics,
            group_by,
            where_,
            order_by,
            limit,
            grain,
        } => {
            let gb_inputs = build_group_by(group_by, grain);
            let ob_inputs = build_order_by(order_by, metrics, &gb_inputs);
            let where_inputs = build_where(where_);
            semantic_layer::queries::execute_query(
                client,
                &host,
                env_id,
                metrics,
                &gb_inputs,
                &where_inputs,
                &ob_inputs,
                *limit,
            )
            .await
        }
        MetricsCommand::Sql {
            metrics,
            group_by,
            where_,
            order_by,
            limit,
            grain,
        } => {
            let gb_inputs = build_group_by(group_by, grain);
            let ob_inputs = build_order_by(order_by, metrics, &gb_inputs);
            let where_inputs = build_where(where_);
            semantic_layer::queries::compile_sql(
                client,
                &host,
                env_id,
                metrics,
                &gb_inputs,
                &where_inputs,
                &ob_inputs,
                *limit,
            )
            .await
        }
    }
}
