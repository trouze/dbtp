use clap::{Args, Subcommand};
use serde_json::{json, Value};

use crate::api::admin::{self, runs};
use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::rest_client::RestClient;

#[derive(Debug, Args)]
pub struct RunsArgs {
    #[command(subcommand)]
    pub command: RunsCommand,
}

#[derive(Debug, Subcommand)]
pub enum RunsCommand {
    /// List runs
    List {
        #[arg(long)]
        job_id: Option<u64>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        limit: Option<u64>,
    },
    /// Show run details
    Show { id: u64 },
    /// Cancel a running run
    Cancel { id: u64 },
    /// Retry a failed run
    Retry { id: u64 },
    /// Show errors from a run (fetches run steps and extracts failures)
    Errors { id: u64 },
}

pub async fn exec(args: &RunsArgs, client: &RestClient, config: &Config) -> Result<Value> {
    let is_compact = config.output == "compact";
    let is_table = config.output == "table" || config.output.is_empty();

    match &args.command {
        RunsCommand::List {
            job_id,
            status,
            limit,
        } => {
            let mut params = Vec::new();
            if let Some(jid) = job_id {
                params.push(("job_definition_id".into(), jid.to_string()));
            }
            if let Some(s) = status {
                params.push(("status".into(), s.clone()));
            }
            let results = runs::list(client, &params, *limit).await?;
            let val = Value::Array(results);
            Ok(if is_table {
                admin::table_view(&val, admin::RUNS_TABLE_FIELDS)
            } else if is_compact {
                admin::compact_runs(&val)
            } else {
                val
            })
        }
        RunsCommand::Show { id } => {
            let val = runs::get(client, *id).await?;
            Ok(if is_compact {
                admin::compact_run(&val)
            } else {
                val
            })
        }
        RunsCommand::Cancel { id } => runs::cancel(client, *id).await,
        RunsCommand::Retry { id } => runs::retry(client, *id).await,
        RunsCommand::Errors { id } => {
            let run = runs::get_with_steps(client, *id).await?;
            Ok(extract_run_errors(&run))
        }
    }
}

fn extract_run_errors(run: &Value) -> Value {
    let mut error_steps = Vec::new();

    if let Some(steps) = run.get("run_steps").and_then(|s| s.as_array()) {
        for step in steps {
            let step_status = step["status_humanized"].as_str().unwrap_or("");
            let is_error = step_status.contains("Error")
                || step_status.contains("Fail")
                || step_status == "Cancelled";

            if is_error {
                let mut info = json!({
                    "index": step["index"],
                    "name": step["name"],
                    "status": step_status,
                });
                if let Some(logs) = step["logs"].as_str() {
                    let lines: Vec<&str> = logs.lines().collect();
                    let relevant = if lines.len() > 50 {
                        lines[lines.len() - 50..].join("\n")
                    } else {
                        logs.to_string()
                    };
                    info["logs"] = json!(relevant);
                }
                error_steps.push(info);
            }
        }
    }

    json!({
        "run_id": run["id"],
        "status": run["status_humanized"],
        "error_steps": error_steps,
    })
}
