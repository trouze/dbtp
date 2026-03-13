use clap::Args;

use crate::core::config::{self, Profile};
use crate::core::error::Result;

#[derive(Debug, Args)]
pub struct ConfigureArgs {
    /// Profile name to configure
    #[arg(long, default_value = "default")]
    pub profile_name: String,
}

pub async fn exec(args: &ConfigureArgs) -> Result<()> {
    eprintln!("Configuring profile: {}\n", args.profile_name);

    let host = config::prompt("dbt Cloud host", "https://cloud.getdbt.com");
    let token = config::prompt("API token", "");
    let account_id_str = config::prompt("Account ID", "");
    let environment_id_str = config::prompt("Environment ID (optional)", "");

    let account_id = if account_id_str.is_empty() {
        None
    } else {
        Some(account_id_str.parse::<u64>().map_err(|_| {
            crate::core::error::DbtpError::config("Account ID must be a number")
        })?)
    };

    let environment_id = if environment_id_str.is_empty() {
        None
    } else {
        Some(environment_id_str.parse::<u64>().map_err(|_| {
            crate::core::error::DbtpError::config("Environment ID must be a number")
        })?)
    };

    let host = if !host.starts_with("https://") && !host.starts_with("http://") {
        format!("https://{host}")
    } else {
        host
    };

    let profile = Profile {
        host: Some(host),
        token: Some(token),
        account_id,
        environment_id,
    };

    config::save_profile(&args.profile_name, &profile)?;

    let path = config::config_path()?;
    eprintln!("\nConfiguration saved to {}", path.display());

    Ok(())
}
