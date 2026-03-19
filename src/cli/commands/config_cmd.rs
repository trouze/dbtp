use clap::{Args, Subcommand};

use crate::core::config;
use crate::core::error::Result;

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Show all current settings
    List,
    /// Get the value of a single setting
    Get {
        /// Key name: host, token, account-id, project-id, output
        key: String,
    },
    /// Set a configuration property
    Set {
        /// Key name: host, token, account-id, project-id, output
        key: String,
        /// Value to set
        value: String,
    },
    /// Clear a configuration property
    Unset {
        /// Key name: host, token, account-id, project-id, output
        key: String,
    },
}

pub async fn exec(args: &ConfigArgs) -> Result<()> {
    match &args.command {
        ConfigCommand::List => {
            let path = config::config_path()?;
            if !path.exists() {
                eprintln!("No config file found at {}", path.display());
                eprintln!("Run `dbtp init` to set up credentials.");
                return Ok(());
            }

            let keys = ["host", "token", "account-id", "project-id", "output"];
            for key in &keys {
                match config::get_property(key)? {
                    Some(val) => {
                        let display = if *key == "token" { mask_token(&val) } else { val };
                        println!("{key} = {display}");
                    }
                    None => println!("{key} = (not set)"),
                }
            }
        }

        ConfigCommand::Get { key } => match config::get_property(key)? {
            Some(val) => {
                let display = if key == "token" { mask_token(&val) } else { val };
                println!("{display}");
            }
            None => eprintln!("{key} is not set"),
        },

        ConfigCommand::Set { key, value } => {
            config::set_property(key, value)?;
            eprintln!("Set {key} = {}", if key == "token" { mask_token(value) } else { value.clone() });
        }

        ConfigCommand::Unset { key } => {
            config::unset_property(key)?;
            eprintln!("Unset {key}");
        }
    }

    Ok(())
}

fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        return "***".to_string();
    }
    format!("{}...{}", &token[..4], &token[token.len() - 4..])
}
