use clap::Args;

use crate::api::admin::{accounts, projects};
use crate::core::config::{self, ConfigFile, Connection, Defaults};
use crate::core::error::Result;
use crate::core::rest_client::RestClient;

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Disable interactive selection menus (use plain prompts only)
    #[arg(long)]
    pub non_interactive: bool,
}

pub async fn exec(args: &InitArgs) -> Result<()> {
    eprintln!("Setting up dbtp credentials.\n");

    let host = config::prompt("dbt Cloud host", "https://cloud.getdbt.com");
    let token = config::prompt("API token", "");

    let host = if !host.starts_with("https://") && !host.starts_with("http://") {
        format!("https://{host}")
    } else {
        host
    };

    let account_id_str = config::prompt("Account ID", "");
    let account_id = account_id_str
        .parse::<u64>()
        .map_err(|_| crate::core::error::DbtpError::config("Account ID must be a number"))?;

    // Validate the connection
    eprint!("\nValidating connection...");
    match accounts::get_by_id(&host, &token, account_id).await {
        Ok(_) => eprintln!(" ok"),
        Err(e) => {
            eprintln!(" failed\nWarning: could not verify credentials: {e}");
            eprintln!("Saving anyway. Run `dbtp accounts show` to verify later.\n");
        }
    }

    let project_id = if !args.non_interactive && config::is_interactive() && !token.is_empty() {
        pick_project(&host, &token, account_id).await
    } else {
        None
    };

    let file = ConfigFile {
        connection: Connection {
            host: Some(host),
            token: Some(token),
            account_id: Some(account_id),
        },
        defaults: Defaults {
            project_id,
            output: "table".to_string(),
        },
    };

    config::save_config(&file)?;

    let path = config::config_path()?;
    eprintln!("\nConfiguration saved to {}", path.display());
    eprintln!("Run `dbtp config set project-id <id-or-name>` to set a default project.");

    Ok(())
}

async fn pick_project(host: &str, token: &str, account_id: u64) -> Option<String> {
    let client = match RestClient::new(host, token, Some(account_id)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\nCould not build API client: {e}");
            return None;
        }
    };

    eprintln!("\nFetching projects...");
    let list = match projects::list(&client, &[], None).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Could not fetch projects: {e}");
            return None;
        }
    };

    if list.is_empty() {
        eprintln!("  No projects found.");
        return None;
    }

    let items: Vec<(String, String)> = list
        .iter()
        .filter_map(|p| {
            let name = p["name"].as_str()?.to_string();
            let id = p["id"].as_u64()?.to_string();
            Some((name, id))
        })
        .collect();

    config::prompt_select("Select default project", &items)
}
