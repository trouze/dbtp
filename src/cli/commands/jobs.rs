use clap::{Args, Subcommand};
use serde_json::{json, Value};

use crate::api::admin::{self, jobs};
use crate::core::config::Config;
use crate::core::error::Result;
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
        #[arg(long)]
        project_id: Option<u64>,
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
        #[arg(long)]
        project_id: u64,
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
    },
    /// Rerun a job from its point of failure
    #[command(name = "trigger-from-failure")]
    TriggerFromFailure { id: u64 },
}

pub async fn exec(args: &JobsArgs, client: &RestClient, config: &Config) -> Result<Value> {
    let is_compact = config.output == "compact";

    match &args.command {
        JobsCommand::List {
            project_id,
            environment_id,
            limit,
        } => {
            let mut params = Vec::new();
            if let Some(pid) = project_id {
                params.push(("project_id".into(), pid.to_string()));
            }
            if let Some(eid) = environment_id {
                params.push(("environment_id".into(), eid.to_string()));
            }
            let results = jobs::list(client, &params, *limit).await?;
            let val = Value::Array(results);
            Ok(if is_compact {
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
            let body = json!({
                "name": name,
                "project_id": project_id,
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
            jobs::trigger(client, *id, &body).await
        }
        JobsCommand::TriggerFromFailure { id } => {
            jobs::trigger_from_failure(client, *id).await
        }
    }
}
