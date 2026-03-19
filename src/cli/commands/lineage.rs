use std::io::BufRead;

use clap::{Args, Subcommand};
use serde_json::Value;

use crate::api::discovery::{column_lineage, lineage, require_environment_id};
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

    /// Analyze cross-project column impact (designed for CI)
    #[command(
        long_about = "Analyze cross-project column impact for CI workflows.\n\n\
            Accepts model unique IDs as positional arguments, or file paths via --files\n\
            (read from stdin). For each model, fetches column-level lineage and identifies\n\
            downstream columns in other projects that would be impacted.\n\n\
            When using --files, pipe file paths from git diff:\n  \
            git diff --name-only origin/main...HEAD -- '*.sql' | dbtp lineage impact --files",
        after_long_help = "EXAMPLES:\n  \
            dbtp lineage impact model.analytics.orders --cross-project-only -o json\n  \
            dbtp lineage impact model.analytics.orders model.analytics.customers \\\n    \
                --cross-project-only --fail-on-impact\n  \
            git diff --name-only origin/main -- '*.sql' \\\n    \
                | dbtp lineage impact --files --cross-project-only --fail-on-impact"
    )]
    Impact {
        /// Model unique IDs (omit when using --files)
        unique_ids: Vec<String>,

        /// Read file paths from stdin and resolve to model unique IDs
        #[arg(long)]
        files: bool,

        /// Only show impacts in other projects
        #[arg(long)]
        cross_project_only: bool,

        /// Exit with code 1 if any impacts are found
        #[arg(long)]
        fail_on_impact: bool,

        /// Downstream environment IDs to search for cross-project consumers.
        /// When omitted, all deployment environments in the account are searched.
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
            unique_ids,
            files,
            cross_project_only,
            fail_on_impact,
            downstream_envs,
        } => {
            let resolved_ids = if *files {
                let stdin = std::io::stdin();
                let paths: Vec<String> = stdin
                    .lock()
                    .lines()
                    .filter_map(|line| {
                        let l = line.ok()?;
                        let trimmed = l.trim().to_string();
                        if trimmed.is_empty() { None } else { Some(trimmed) }
                    })
                    .collect();

                if paths.is_empty() {
                    return Err(DbtpError::config(
                        "no file paths provided on stdin; pipe file paths with --files",
                    ));
                }

                let (resolved, unmatched) = column_lineage::resolve_files_to_unique_ids(
                    client,
                    &config.host,
                    env_id,
                    &paths,
                )
                .await?;

                for path in &unmatched {
                    eprintln!("warning: could not resolve file to a model: {path}");
                }

                if resolved.is_empty() {
                    return Err(DbtpError::config(
                        "none of the provided file paths matched any models in the environment",
                    ));
                }

                resolved
            } else {
                if unique_ids.is_empty() {
                    return Err(DbtpError::config(
                        "provide model unique IDs as arguments, or use --files to read file paths from stdin",
                    ));
                }
                unique_ids.clone()
            };

            let report = column_lineage::build_impact_report(
                client,
                rest,
                &config.host,
                env_id,
                &resolved_ids,
                *cross_project_only,
                downstream_envs,
            )
            .await?;

            if *fail_on_impact {
                let impact_count = report["summary"]["total_impacts"]
                    .as_u64()
                    .unwrap_or(0);
                if impact_count > 0 {
                    return Err(DbtpError::ImpactFound(report));
                }
            }

            Ok(report)
        }
    }
}
