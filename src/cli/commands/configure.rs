use clap::Args;

use crate::api::admin::{environments, projects};
use crate::core::config::{self, Profile};
use crate::core::error::Result;
use crate::core::rest_client::RestClient;

#[derive(Debug, Args)]
pub struct ConfigureArgs {
    /// Profile name to configure
    #[arg(long, default_value = "default")]
    pub profile_name: String,

    /// Disable interactive selection menus (use plain prompts only)
    #[arg(long)]
    pub non_interactive: bool,
}

pub async fn exec(args: &ConfigureArgs) -> Result<()> {
    eprintln!("Configuring profile: {}\n", args.profile_name);

    let host = config::prompt("dbt Cloud host", "https://cloud.getdbt.com");
    let token = config::prompt("API token", "");
    let account_id_str = config::prompt("Account ID", "");

    let host = if !host.starts_with("https://") && !host.starts_with("http://") {
        format!("https://{host}")
    } else {
        host
    };

    let account_id = if account_id_str.is_empty() {
        None
    } else {
        Some(account_id_str.parse::<u64>().map_err(|_| {
            crate::core::error::DbtpError::config("Account ID must be a number")
        })?)
    };

    let interactive = !args.non_interactive && config::is_interactive();
    let (project_id, environment_id) = if interactive && !token.is_empty() && account_id.is_some()
    {
        pick_project_and_environment(&host, &token, account_id.unwrap()).await
    } else {
        prompt_ids_manually()
    };

    let profile = Profile {
        host: Some(host),
        token: Some(token),
        account_id,
        project_id,
        environment_id,
    };

    config::save_profile(&args.profile_name, &profile)?;

    let path = config::config_path()?;
    eprintln!("\nConfiguration saved to {}", path.display());

    Ok(())
}

fn prompt_ids_manually() -> (Option<String>, Option<String>) {
    let project_id_str = config::prompt("Project ID (optional)", "");
    let environment_id_str = config::prompt("Environment ID (optional)", "");

    let project_id = if project_id_str.is_empty() {
        None
    } else {
        Some(project_id_str)
    };

    let environment_id = if environment_id_str.is_empty() {
        None
    } else {
        Some(environment_id_str)
    };

    (project_id, environment_id)
}

async fn pick_project_and_environment(
    host: &str,
    token: &str,
    account_id: u64,
) -> (Option<String>, Option<String>) {
    let client = match RestClient::new(host, token, Some(account_id)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\nCould not build API client: {e}. Enter IDs manually.");
            return prompt_ids_manually();
        }
    };

    let project_id = match pick_project(&client).await {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Could not fetch projects: {e}. Enter project ID manually.");
            let s = config::prompt("Project ID (optional)", "");
            if s.is_empty() { None } else { Some(s) }
        }
    };

    let environment_id = if let Some(ref pid_str) = project_id {
        if let Ok(pid) = pid_str.parse::<u64>() {
            match pick_environment(&client, pid).await {
                Ok(id) => id,
                Err(e) => {
                    eprintln!("Could not fetch environments: {e}. Enter environment ID manually.");
                    let s = config::prompt("Environment ID (optional)", "");
                    if s.is_empty() { None } else { Some(s) }
                }
            }
        } else {
            let s = config::prompt("Environment ID (optional)", "");
            if s.is_empty() { None } else { Some(s) }
        }
    } else {
        let s = config::prompt("Environment ID (optional)", "");
        if s.is_empty() { None } else { Some(s) }
    };

    (project_id, environment_id)
}

async fn pick_project(
    client: &RestClient,
) -> std::result::Result<Option<String>, Box<dyn std::error::Error>> {
    eprintln!("\nFetching projects...");
    let list = projects::list(client, &[], None).await?;

    if list.is_empty() {
        eprintln!("  No projects found.");
        return Ok(None);
    }

    let items: Vec<(String, String)> = list
        .iter()
        .filter_map(|p| {
            let name = p["name"].as_str()?.to_string();
            let id = p["id"].as_u64()?.to_string();
            Some((name, id))
        })
        .collect();

    Ok(config::prompt_select("Select project", &items))
}

async fn pick_environment(
    client: &RestClient,
    project_id: u64,
) -> std::result::Result<Option<String>, Box<dyn std::error::Error>> {
    eprintln!("\nFetching environments...");
    let list = environments::list(client, project_id, &[], None).await?;

    if list.is_empty() {
        eprintln!("  No environments found.");
        return Ok(None);
    }

    let items: Vec<(String, String)> = list
        .iter()
        .filter_map(|e| {
            let name = e["name"].as_str()?.to_string();
            let id = e["id"].as_u64()?.to_string();
            let env_type = e["type"].as_str().unwrap_or("unknown");
            Some((format!("{name}, {env_type}"), id))
        })
        .collect();

    Ok(config::prompt_select("Select environment", &items))
}
