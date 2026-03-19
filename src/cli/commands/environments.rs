use clap::{Args, Subcommand};
use serde_json::{json, Value};

use crate::api::admin::{self, environments};
use crate::core::config::Config;
use crate::core::error::{DbtpError, Result};
use crate::core::resolve;
use crate::core::rest_client::RestClient;

#[derive(Debug, Args)]
pub struct EnvironmentsArgs {
    /// Project ID or name (falls back to config/env var)
    #[arg(long)]
    pub project_id: Option<String>,

    #[command(subcommand)]
    pub command: EnvironmentsCommand,
}

#[derive(Debug, Subcommand)]
pub enum EnvironmentsCommand {
    /// List environments in a project
    List {
        #[arg(long)]
        limit: Option<u64>,
    },
    /// Show environment details
    Show { id: u64 },
    /// Create a new environment
    Create {
        #[arg(long)]
        name: String,
        /// Environment type
        #[arg(long, value_parser = ["deployment", "development"])]
        r#type: String,
    },
    /// Update an environment
    Update {
        id: u64,
        #[arg(long)]
        name: Option<String>,
    },
    /// Delete an environment
    Delete { id: u64 },
}

pub async fn exec(
    args: &EnvironmentsArgs,
    client: &RestClient,
    config: &Config,
) -> Result<Value> {
    let raw_pid = args
        .project_id
        .as_deref()
        .or(config.project_id.as_deref())
        .ok_or_else(|| {
            DbtpError::config(
                "project_id is required for environment operations; \
                 set via --project-id, DBTP_PROJECT_ID, or `dbtp config set project-id`",
            )
        })?;
    let pid = resolve::resolve_project(client, raw_pid).await?;
    let is_table = config.output == "table" || config.output.is_empty();

    match &args.command {
        EnvironmentsCommand::List { limit } => {
            let results = environments::list(client, pid, &[], *limit).await?;
            let val = Value::Array(results);
            Ok(if is_table {
                admin::table_view(&val, admin::ENVIRONMENTS_TABLE_FIELDS)
            } else {
                val
            })
        }
        EnvironmentsCommand::Show { id } => environments::get(client, pid, *id).await,
        EnvironmentsCommand::Create { name, r#type } => {
            let body = json!({
                "name": name,
                "type": r#type,
                "project_id": pid,
            });
            environments::create(client, pid, &body).await
        }
        EnvironmentsCommand::Update { id, name } => {
            let mut body = json!({});
            if let Some(n) = name {
                body["name"] = json!(n);
            }
            environments::update(client, pid, *id, &body).await
        }
        EnvironmentsCommand::Delete { id } => environments::delete(client, pid, *id).await,
    }
}
