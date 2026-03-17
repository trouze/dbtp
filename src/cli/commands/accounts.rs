use clap::{Args, Subcommand};
use serde_json::Value;

use crate::api::admin::{self, accounts};
use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::rest_client::RestClient;

#[derive(Debug, Args)]
pub struct AccountsArgs {
    #[command(subcommand)]
    pub command: AccountsCommand,
}

#[derive(Debug, Subcommand)]
pub enum AccountsCommand {
    /// List accessible accounts
    List,
    /// Show account details
    Show {
        /// Account ID (defaults to configured account)
        account_id: Option<u64>,
    },
}

pub async fn exec(args: &AccountsArgs, client: &RestClient, config: &Config) -> Result<Value> {
    let is_table = config.output == "table" || config.output.is_empty();

    match &args.command {
        AccountsCommand::List => {
            let val = accounts::list(&config.host, &config.token).await?;
            Ok(if is_table {
                admin::table_view(&val, admin::ACCOUNTS_TABLE_FIELDS)
            } else {
                val
            })
        }
        AccountsCommand::Show { account_id } => match account_id {
            Some(id) => accounts::get_by_id(&config.host, &config.token, *id).await,
            None => accounts::get(client).await,
        },
    }
}
