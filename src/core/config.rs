use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use super::error::{DbtpError, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub token: String,
    /// Service token for the Semantic Layer API. Falls back to `token` if not set.
    pub service_token: Option<String>,
    pub account_id: Option<u64>,
    pub project_id: Option<String>,
    pub environment_id: Option<String>,
    pub output: String,
}

impl Config {
    pub fn project_id_u64(&self) -> Option<u64> {
        self.project_id.as_ref().and_then(|s| s.parse().ok())
    }

    pub fn environment_id_u64(&self) -> Option<u64> {
        self.environment_id.as_ref().and_then(|s| s.parse().ok())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigFile {
    #[serde(default)]
    pub connection: Connection,
    #[serde(default)]
    pub defaults: Defaults,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Connection {
    pub host: Option<String>,
    pub token: Option<String>,
    pub service_token: Option<String>,
    pub account_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Defaults {
    pub project_id: Option<String>,
    #[serde(default = "default_output")]
    pub output: String,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            project_id: None,
            output: default_output(),
        }
    }
}

fn default_output() -> String {
    "table".into()
}

pub struct ConfigOverrides {
    pub host: Option<String>,
    pub token: Option<String>,
    pub service_token: Option<String>,
    pub account_id: Option<u64>,
    pub project_id: Option<String>,
    pub environment_id: Option<String>,
    pub output: Option<String>,
}

pub fn config_dir() -> Result<PathBuf> {
    ProjectDirs::from("com", "dbt-labs", "dbtp")
        .map(|dirs| dirs.config_dir().to_path_buf())
        .ok_or_else(|| DbtpError::config("Could not determine config directory"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

fn load_config_file() -> Result<ConfigFile> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(ConfigFile::default());
    }
    let contents = std::fs::read_to_string(&path).map_err(DbtpError::Io)?;

    // Migration: detect old profile-based format
    let parsed: toml::Value = toml::from_str(&contents)
        .map_err(|e| DbtpError::config(format!("Invalid config file: {e}")))?;

    if parsed.get("profile").is_some() {
        return migrate_config(&parsed, &path);
    }

    toml::from_str(&contents).map_err(|e| DbtpError::config(format!("Invalid config file: {e}")))
}

fn migrate_config(parsed: &toml::Value, path: &PathBuf) -> Result<ConfigFile> {
    let profile_name = parsed
        .get("defaults")
        .and_then(|d| d.get("profile"))
        .and_then(|p| p.as_str())
        .unwrap_or("default");

    let profile_table = parsed.get("profile");
    let profile = profile_table
        .and_then(|t| t.get(profile_name))
        .or_else(|| {
            profile_table
                .and_then(|t| t.as_table())
                .and_then(|t| t.values().next())
        });

    let mut new_config = ConfigFile::default();

    if let Some(p) = profile {
        new_config.connection.host =
            p.get("host").and_then(|v| v.as_str()).map(String::from);
        new_config.connection.token =
            p.get("token").and_then(|v| v.as_str()).map(String::from);
        new_config.connection.account_id =
            p.get("account_id").and_then(|v| v.as_integer()).map(|i| i as u64);
        new_config.defaults.project_id =
            p.get("project_id").and_then(|v| v.as_str()).map(String::from);
    }

    new_config.defaults.output = parsed
        .get("defaults")
        .and_then(|d| d.get("output"))
        .and_then(|o| o.as_str())
        .unwrap_or("table")
        .to_string();

    let dir = path.parent().unwrap();
    std::fs::create_dir_all(dir).map_err(DbtpError::Io)?;
    let contents = toml::to_string_pretty(&new_config)
        .map_err(|e| DbtpError::config(format!("TOML error: {e}")))?;
    std::fs::write(path, contents).map_err(DbtpError::Io)?;

    eprintln!("Migrated config from profile-based format. Old profiles have been consolidated.");

    Ok(new_config)
}

/// Load configuration with precedence: CLI flags > env vars > config file > defaults
pub fn load(overrides: ConfigOverrides) -> Result<Config> {
    let file = load_config_file()?;

    let host = normalize_host(
        overrides
            .host
            .or_else(|| std::env::var("DBTP_HOST").ok())
            .or(file.connection.host)
            .unwrap_or_else(|| "https://cloud.getdbt.com".into()),
    );

    let token = overrides
        .token
        .or_else(|| std::env::var("DBTP_TOKEN").ok())
        .or(file.connection.token)
        .unwrap_or_default();

    let service_token = overrides
        .service_token
        .or_else(|| std::env::var("DBTP_SERVICE_TOKEN").ok())
        .or(file.connection.service_token);

    let account_id = overrides
        .account_id
        .or_else(|| {
            std::env::var("DBTP_ACCOUNT_ID")
                .ok()
                .and_then(|v| v.parse().ok())
        })
        .or(file.connection.account_id);

    let project_id = overrides
        .project_id
        .or_else(|| std::env::var("DBTP_PROJECT_ID").ok())
        .or(file.defaults.project_id);

    let environment_id = overrides
        .environment_id
        .or_else(|| std::env::var("DBTP_ENVIRONMENT_ID").ok());

    let output = overrides.output.unwrap_or(file.defaults.output);

    Ok(Config {
        host,
        token,
        service_token,
        account_id,
        project_id,
        environment_id,
        output,
    })
}

pub fn save_config(file: &ConfigFile) -> Result<()> {
    let path = config_path()?;
    let dir = path.parent().unwrap();
    std::fs::create_dir_all(dir).map_err(DbtpError::Io)?;

    let contents = toml::to_string_pretty(file)
        .map_err(|e| DbtpError::config(format!("TOML error: {e}")))?;
    std::fs::write(&path, contents).map_err(DbtpError::Io)?;

    Ok(())
}

pub fn set_property(key: &str, value: &str) -> Result<()> {
    let mut file = load_config_file()?;

    match key {
        "host" => file.connection.host = Some(normalize_host(value.to_string())),
        "token" => file.connection.token = Some(value.to_string()),
        "account-id" => {
            let id = value
                .parse::<u64>()
                .map_err(|_| DbtpError::config("account-id must be a positive integer"))?;
            file.connection.account_id = Some(id);
        }
        "project-id" => file.defaults.project_id = Some(value.to_string()),
        "output" => match value {
            "table" | "json" | "yaml" | "compact" => file.defaults.output = value.to_string(),
            _ => {
                return Err(DbtpError::config(
                    "output must be one of: table, json, yaml, compact",
                ))
            }
        },
        _ => {
            return Err(DbtpError::config(format!(
                "Unknown key \"{key}\". Valid keys: host, token, account-id, project-id, output"
            )))
        }
    }

    save_config(&file)
}

pub fn unset_property(key: &str) -> Result<()> {
    let mut file = load_config_file()?;

    match key {
        "host" => file.connection.host = None,
        "token" => file.connection.token = None,
        "account-id" => file.connection.account_id = None,
        "project-id" => file.defaults.project_id = None,
        "output" => file.defaults.output = default_output(),
        _ => {
            return Err(DbtpError::config(format!(
                "Unknown key \"{key}\". Valid keys: host, token, account-id, project-id, output"
            )))
        }
    }

    save_config(&file)
}

pub fn get_property(key: &str) -> Result<Option<String>> {
    let file = load_config_file()?;

    let val = match key {
        "host" => file.connection.host,
        "token" => file.connection.token,
        "account-id" => file.connection.account_id.map(|v| v.to_string()),
        "project-id" => file.defaults.project_id,
        "output" => Some(file.defaults.output),
        _ => {
            return Err(DbtpError::config(format!(
                "Unknown key \"{key}\". Valid keys: host, token, account-id, project-id, output"
            )))
        }
    };

    Ok(val)
}

fn normalize_host(host: String) -> String {
    let h = host.trim().trim_end_matches('/').to_string();
    if h.starts_with("https://") || h.starts_with("http://") {
        h
    } else {
        format!("https://{h}")
    }
}

pub fn prompt(label: &str, default: &str) -> String {
    if default.is_empty() {
        eprint!("{label}: ");
    } else {
        eprint!("{label} [{default}]: ");
    }
    io::stderr().flush().ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    let trimmed = input.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn is_interactive() -> bool {
    io::stdin().is_terminal() && io::stderr().is_terminal()
}

/// Display a numbered selection menu and return the chosen value.
/// `items` is a slice of `(display_label, id_value)` pairs.
/// Returns `Some(id_value)` if the user picks an item, or `None` if they skip.
pub fn prompt_select(label: &str, items: &[(String, String)]) -> Option<String> {
    if items.is_empty() {
        return None;
    }
    for (i, (display, id)) in items.iter().enumerate() {
        eprintln!("  [{}] {} ({})", i + 1, display, id);
    }
    eprintln!("  [s] Skip");
    let choice = prompt(label, "s");
    if choice.eq_ignore_ascii_case("s") || choice.is_empty() {
        return None;
    }
    choice
        .parse::<usize>()
        .ok()
        .filter(|&n| n >= 1 && n <= items.len())
        .map(|n| items[n - 1].1.clone())
}
