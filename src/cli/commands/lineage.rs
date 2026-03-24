use std::io::BufRead;

use clap::{Args, Subcommand};
use serde_json::Value;

use serde_json::json;

use crate::api::discovery::{column_lineage, lineage, require_environment_id};
use crate::cli::output::OutputFormat;
use crate::core::config::Config;
use crate::core::error::{DbtpError, Result};
use crate::core::graphql_client::GraphqlClient;
use crate::core::rest_client::RestClient;

#[derive(Debug, Args)]
pub struct LineageArgs {
    #[command(subcommand)]
    pub command: LineageCommand,
}

#[derive(Debug, Subcommand)]
pub enum LineageCommand {
    /// Show lineage graph for a node (upstream + downstream)
    Show {
        /// Unique ID of the resource (e.g. "model.analytics.orders")
        unique_id: String,

        /// Max hops from the target node (0 = unlimited)
        #[arg(long, default_value = "5")]
        depth: u32,

        /// Resource types to include (e.g. Model, Source, Exposure)
        #[arg(long, value_delimiter = ',')]
        types: Vec<String>,
    },

    /// Show column-level lineage for a model
    #[command(
        long_about = "Show column-level lineage for a model.\n\n\
            Fetches the full column-level dependency graph reachable from the given model,\n\
            including columns in other projects. Each column shows its parent/child\n\
            relationships, transformation type, and project ID.",
        after_long_help = "EXAMPLES:\n  \
            dbtp lineage columns model.analytics.orders\n  \
            dbtp lineage columns model.analytics.orders --downstream-only -o json"
    )]
    Columns {
        /// Model unique ID (e.g. "model.analytics.orders")
        unique_id: String,

        /// Only show downstream columns (depth >= 0)
        #[arg(long)]
        downstream_only: bool,

        /// Only show upstream columns (depth <= 0)
        #[arg(long, conflicts_with = "downstream_only")]
        upstream_only: bool,
    },

    /// Analyze public-model column impact of changed models (designed for CI)
    ///
    /// For each changed model, walks node-level lineage to find downstream public
    /// models, then traces column-level lineage to report exactly which columns
    /// from the changed model land in those public models.
    ///
    /// Inputs can be model unique IDs (model.project.name) or relative file paths
    /// (models/staging/stg_teams.sql). Use --files to read paths from stdin.
    #[command(after_long_help = "EXAMPLES:\n  \
            dbtp lineage impact model.analytics.orders -o json\n  \
            dbtp lineage impact models/staging/stg_teams.sql\n  \
            git diff --name-only origin/main -- '*.sql' \\\n    \
                | dbtp lineage impact --files --fail-on-impact")]
    Impact {
        /// Model unique IDs or relative file paths (omit when using --files)
        inputs: Vec<String>,

        /// Read file paths from stdin instead of positional arguments
        #[arg(long)]
        files: bool,

        /// Only show impacts that have at least one cross-project consumer
        #[arg(long)]
        cross_project: bool,

        /// Exit with code 1 if any public-model impacts are found
        #[arg(long)]
        fail_on_impact: bool,

        /// Downstream environment IDs to search for cross-project consumers (comma-separated).
        /// When omitted, all deployment environments in the account are searched (slower).
        #[arg(long, value_delimiter = ',')]
        downstream_envs: Vec<u64>,
    },
}

pub async fn exec(
    args: &LineageArgs,
    client: &GraphqlClient,
    rest: &RestClient,
    config: &Config,
) -> Result<Value> {
    let env_id = require_environment_id(config)?;

    match &args.command {
        LineageCommand::Show {
            unique_id,
            depth,
            types,
        } => lineage::fetch(client, &config.host, env_id, unique_id, *depth, types).await,

        LineageCommand::Columns {
            unique_id,
            downstream_only,
            upstream_only,
        } => {
            column_lineage::fetch(
                client,
                &config.host,
                env_id,
                unique_id,
                *downstream_only,
                *upstream_only,
            )
            .await
        }

        LineageCommand::Impact {
            inputs,
            files,
            cross_project,
            fail_on_impact,
            downstream_envs,
        } => {
            // Collect raw inputs: either from stdin (--files) or positional args.
            let raw_inputs: Vec<String> = if *files {
                let stdin = std::io::stdin();
                let paths: Vec<String> = stdin
                    .lock()
                    .lines()
                    .filter_map(|line| {
                        let l = line.ok()?;
                        let trimmed = l.trim().to_string();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed)
                        }
                    })
                    .collect();

                if paths.is_empty() {
                    return Err(DbtpError::config(
                        "no file paths provided on stdin; pipe file paths with --files",
                    ));
                }
                paths
            } else {
                if inputs.is_empty() {
                    return Err(DbtpError::config(
                        "provide model unique IDs or file paths as arguments, \
                         or use --files to read from stdin",
                    ));
                }
                inputs.clone()
            };

            // Partition inputs into unique IDs (model.project.name) and file paths.
            let mut resolved_ids: Vec<String> = Vec::new();
            let mut file_paths: Vec<String> = Vec::new();

            for input in &raw_inputs {
                if is_file_path(input) {
                    file_paths.push(input.clone());
                } else {
                    resolved_ids.push(input.clone());
                }
            }

            // Resolve file paths to unique IDs via the Discovery API.
            if !file_paths.is_empty() {
                let (resolved, unmatched) = column_lineage::resolve_files_to_unique_ids(
                    client,
                    &config.host,
                    env_id,
                    &file_paths,
                )
                .await?;

                for path in &unmatched {
                    eprintln!("warning: could not resolve file to a model: {path}");
                }

                resolved_ids.extend(resolved);
            }

            if resolved_ids.is_empty() {
                return Err(DbtpError::config(
                    "none of the provided inputs matched any models in the environment",
                ));
            }

            // --downstream-envs implies --cross-project; --cross-project without
            // --downstream-envs triggers auto-discovery of deployment environments.
            let effective_cross_project = *cross_project || !downstream_envs.is_empty();
            let effective_envs: &[u64] = if effective_cross_project {
                downstream_envs
            } else {
                &[]
            };

            let mut report = column_lineage::build_impact_report(
                client,
                rest,
                &config.host,
                env_id,
                &resolved_ids,
                effective_envs,
            )
            .await?;

            if effective_cross_project {
                if let Some(impacts) = report["impacts"].as_array_mut() {
                    impacts.retain(|i| i["consumers"].as_array().is_some_and(|c| !c.is_empty()));
                }
                // Recount after filtering
                let filtered_count = report["impacts"].as_array().map_or(0, |a| a.len());
                report["summary"]["total_impacts"] = json!(filtered_count);
            }

            if *fail_on_impact {
                let should_fail = if effective_cross_project {
                    // Cross-project mode: only fail when confirmed consumers are found.
                    report["summary"]["cross_project_consumers"]
                        .as_array()
                        .is_some_and(|a| !a.is_empty())
                } else {
                    // No cross-project lookup: fail if any columns land in a public model.
                    report["summary"]["total_impacts"]
                        .as_u64()
                        .is_some_and(|n| n > 0)
                };
                if should_fail {
                    return Err(DbtpError::ImpactFound(report));
                }
            }

            // For table output, reshape to a per-consumer summary instead of the raw
            // impacts array (which has nested objects the generic formatter can't render).
            if OutputFormat::parse(&config.output) == OutputFormat::Table {
                Ok(consumer_summary_table(&report))
            } else {
                Ok(report)
            }
        }
    }
}

/// Reshape the impact report into a flat array suitable for table display.
///
/// Produces one row per (consumer, public_model) pair with a count of the distinct
/// columns from the source model that flow into that consumer.
fn consumer_summary_table(report: &serde_json::Value) -> serde_json::Value {
    use std::collections::HashMap;

    // key: (consumer_model, public_model, project_id, environment_id)
    let mut counts: HashMap<(String, String, String, String), usize> = HashMap::new();

    if let Some(impacts) = report["impacts"].as_array() {
        for impact in impacts {
            let public_model = impact["public_model"].as_str().unwrap_or("").to_string();
            let consumers = impact["consumers"].as_array();

            if let Some(consumers) = consumers {
                for c in consumers {
                    let key = (
                        c["model"].as_str().unwrap_or("").to_string(),
                        public_model.clone(),
                        c["project_id"]
                            .as_str()
                            .map(String::from)
                            .or_else(|| c["project_id"].as_u64().map(|n| n.to_string()))
                            .unwrap_or_default(),
                        c["environment_id"]
                            .as_u64()
                            .map(|n| n.to_string())
                            .unwrap_or_default(),
                    );
                    *counts.entry(key).or_insert(0) += 1;
                }
            }
        }
    }

    if counts.is_empty() {
        // No cross-project consumers found — show the public model summary instead.
        let public_models: Vec<serde_json::Value> = report["summary"]["public_models_with_impact"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|m| {
                let model = m.as_str().unwrap_or("").to_string();
                let col_count = report["impacts"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter(|i| i["public_model"].as_str() == Some(model.as_str()))
                            .count()
                    })
                    .unwrap_or(0);
                json!({
                    "public_model": model,
                    "impacted_columns": col_count,
                    "consumers": "none found",
                })
            })
            .collect();

        return json!(public_models);
    }

    let mut rows: Vec<serde_json::Value> = counts
        .into_iter()
        .map(
            |((consumer, public_model, project_id, environment_id), col_count)| {
                json!({
                    "consumer_model": consumer,
                    "public_model": public_model,
                    "project_id": project_id,
                    "environment_id": environment_id,
                    "impacted_columns": col_count,
                })
            },
        )
        .collect();

    rows.sort_by(|a, b| {
        a["consumer_model"]
            .as_str()
            .cmp(&b["consumer_model"].as_str())
    });

    json!(rows)
}

/// Returns true if the input looks like a file path rather than a model unique ID.
/// A unique ID has the form `type.project.name` (dots, no slashes).
/// A file path contains a slash or ends with a SQL/Python extension.
fn is_file_path(input: &str) -> bool {
    input.contains('/') || input.ends_with(".sql") || input.ends_with(".py")
}
