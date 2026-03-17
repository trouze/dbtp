use clap::{Args, Subcommand};
use serde_json::{json, Value};

use crate::api::admin::{self, jobs, runs};
use crate::core::config::Config;
use crate::core::error::{DbtpError, Result};
use crate::core::resolve;
use crate::core::rest_client::RestClient;

#[derive(Debug, Args)]
pub struct JobsArgs {
    #[command(subcommand)]
    pub command: JobsCommand,
}

#[derive(Debug, Subcommand)]
pub enum JobsCommand {
    /// List jobs
    List {
        /// Project ID or name (falls back to config)
        #[arg(long)]
        project_id: Option<String>,
        #[arg(long)]
        environment_id: Option<u64>,
        #[arg(long)]
        limit: Option<u64>,
    },
    /// Show job details
    Show { id: u64 },
    /// Create a new job
    Create {
        #[arg(long)]
        name: String,
        /// Project ID or name (falls back to config)
        #[arg(long)]
        project_id: Option<String>,
        #[arg(long)]
        environment_id: u64,
    },
    /// Update a job
    Update {
        id: u64,
        #[arg(long)]
        name: Option<String>,
    },
    /// Delete a job
    Delete { id: u64 },
    /// Trigger a job run
    Trigger {
        id: u64,
        #[arg(long)]
        cause: Option<String>,
        #[arg(long)]
        git_branch: Option<String>,
        #[arg(long)]
        git_sha: Option<String>,
        /// Wait for the triggered run to reach a final state
        #[arg(long)]
        wait: bool,
        /// Polling interval in seconds (used with --wait)
        #[arg(long, default_value = "10")]
        interval: u64,
        /// Timeout in seconds (used with --wait)
        #[arg(long, default_value = "3600")]
        timeout: u64,
    },
    /// Rerun a job from its point of failure
    #[command(name = "trigger-from-failure")]
    TriggerFromFailure {
        id: u64,
        /// Wait for the triggered run to reach a final state
        #[arg(long)]
        wait: bool,
        /// Polling interval in seconds (used with --wait)
        #[arg(long, default_value = "10")]
        interval: u64,
        /// Timeout in seconds (used with --wait)
        #[arg(long, default_value = "3600")]
        timeout: u64,
    },
}

pub async fn exec(args: &JobsArgs, client: &RestClient, config: &Config) -> Result<Value> {
    let is_compact = config.output == "compact";
    let is_table = config.output == "table" || config.output.is_empty();

    match &args.command {
        JobsCommand::List {
            project_id,
            environment_id,
            limit,
        } => {
            let mut params = Vec::new();
            let raw_pid = project_id.as_deref().or(config.project_id.as_deref());
            if let Some(raw) = raw_pid {
                let pid = resolve::resolve_project(client, raw).await?;
                params.push(("project_id".into(), pid.to_string()));
            }
            if let Some(eid) = environment_id {
                params.push(("environment_id".into(), eid.to_string()));
            }
            let results = jobs::list(client, &params, *limit).await?;
            let val = Value::Array(results);
            Ok(if is_table {
                admin::table_view(&val, admin::JOBS_TABLE_FIELDS)
            } else if is_compact {
                admin::compact_jobs(&val)
            } else {
                val
            })
        }
        JobsCommand::Show { id } => jobs::get(client, *id).await,
        JobsCommand::Create {
            name,
            project_id,
            environment_id,
        } => {
            let raw_pid = project_id
                .as_deref()
                .or(config.project_id.as_deref())
                .ok_or_else(|| {
                    DbtpError::config(
                        "project_id is required for job creation; \
                         set via --project-id, DBTP_PROJECT_ID, or `dbtp configure`",
                    )
                })?;
            let pid = resolve::resolve_project(client, raw_pid).await?;
            let body = json!({
                "name": name,
                "project_id": pid,
                "environment_id": environment_id,
            });
            jobs::create(client, &body).await
        }
        JobsCommand::Update { id, name } => {
            let mut body = json!({});
            if let Some(n) = name {
                body["name"] = json!(n);
            }
            jobs::update(client, *id, &body).await
        }
        JobsCommand::Delete { id } => jobs::delete(client, *id).await,
        JobsCommand::Trigger {
            id,
            cause,
            git_branch,
            git_sha,
            wait,
            interval,
            timeout,
        } => {
            let mut body = json!({
                "cause": cause.as_deref().unwrap_or("Triggered via dbtp CLI"),
            });
            if let Some(branch) = git_branch {
                body["git_branch"] = json!(branch);
            }
            if let Some(sha) = git_sha {
                body["git_sha"] = json!(sha);
            }
            let run = jobs::trigger(client, *id, &body).await?;
            if *wait {
                let run_id = run["id"]
                    .as_u64()
                    .ok_or_else(|| DbtpError::config("Trigger response missing run id"))?;
                wait_for_run(client, run_id, *interval, *timeout).await
            } else {
                Ok(run)
            }
        }
        JobsCommand::TriggerFromFailure {
            id,
            wait,
            interval,
            timeout,
        } => {
            let run = jobs::trigger_from_failure(client, *id).await?;
            if *wait {
                let run_id = run["id"]
                    .as_u64()
                    .ok_or_else(|| DbtpError::config("Trigger response missing run id"))?;
                wait_for_run(client, run_id, *interval, *timeout).await
            } else {
                Ok(run)
            }
        }
    }
}

const STATUS_SUCCESS: u64 = 10;
const STATUS_ERROR: u64 = 20;
const STATUS_CANCELLED: u64 = 30;

fn is_terminal_status(status: u64) -> bool {
    matches!(status, STATUS_SUCCESS | STATUS_ERROR | STATUS_CANCELLED)
}

async fn wait_for_run(
    client: &RestClient,
    run_id: u64,
    interval: u64,
    timeout: u64,
) -> Result<Value> {
    eprintln!("Waiting for run {run_id} (polling every {interval}s, timeout {timeout}s)...");
    let start = std::time::Instant::now();
    let mut last_status = String::new();

    loop {
        let run = runs::get(client, run_id).await?;
        let status_code = run["status"].as_u64().unwrap_or(0);
        let status_human = run["status_humanized"]
            .as_str()
            .unwrap_or("Unknown")
            .to_string();

        if status_human != last_status {
            eprintln!("Run {run_id}: {status_human}");
            last_status = status_human;
        }

        if is_terminal_status(status_code) {
            return Ok(run);
        }

        if start.elapsed().as_secs() >= timeout {
            return Err(DbtpError::config(format!(
                "Timeout waiting for run {run_id} after {timeout}s"
            )));
        }

        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
    }
}
