use std::collections::HashMap;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use super::error::{DbtpError, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub token: String,
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
    pub defaults: Defaults,
    #[serde(default)]
    pub profile: HashMap<String, Profile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Defaults {
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(default = "default_output")]
    pub output: String,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            profile: default_profile(),
            output: default_output(),
        }
    }
}

fn default_profile() -> String {
    "default".into()
}

fn default_output() -> String {
    "table".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Profile {
    pub host: Option<String>,
    pub token: Option<String>,
    pub account_id: Option<u64>,
    pub project_id: Option<String>,
    pub environment_id: Option<String>,
}

pub struct ConfigOverrides {
    pub profile: Option<String>,
    pub host: Option<String>,
    pub token: Option<String>,
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
    toml::from_str(&contents).map_err(|e| DbtpError::config(format!("Invalid config file: {e}")))
}

/// Load configuration with precedence: CLI flags > env vars > profile > defaults
pub fn load(overrides: ConfigOverrides) -> Result<Config> {
    let file = load_config_file()?;

    let profile_name = overrides
        .profile
        .unwrap_or_else(|| file.defaults.profile.clone());

    let profile = file.profile.get(&profile_name).cloned().unwrap_or_default();

    let host = normalize_host(
        overrides
            .host
            .or_else(|| std::env::var("DBTP_HOST").ok())
            .or(profile.host)
            .unwrap_or_else(|| "https://cloud.getdbt.com".into()),
    );

    let token = overrides
        .token
        .or_else(|| std::env::var("DBTP_TOKEN").ok())
        .or(profile.token)
        .unwrap_or_default();

    let account_id = overrides.account_id.or_else(|| {
        std::env::var("DBTP_ACCOUNT_ID")
            .ok()
            .and_then(|v| v.parse().ok())
    }).or(profile.account_id);

    let project_id = overrides
        .project_id
        .or_else(|| std::env::var("DBTP_PROJECT_ID").ok())
        .or(profile.project_id);

    let environment_id = overrides
        .environment_id
        .or_else(|| std::env::var("DBTP_ENVIRONMENT_ID").ok())
        .or(profile.environment_id);

    let output = overrides
        .output
        .unwrap_or_else(|| file.defaults.output.clone());

    Ok(Config {
        host,
        token,
        account_id,
        project_id,
        environment_id,
        output,
    })
}

pub fn save_profile(name: &str, profile: &Profile) -> Result<()> {
    let path = config_path()?;
    let dir = path.parent().unwrap();
    std::fs::create_dir_all(dir).map_err(DbtpError::Io)?;

    let mut file = load_config_file()?;
    file.profile.insert(name.to_string(), profile.clone());

    let contents =
        toml::to_string_pretty(&file).map_err(|e| DbtpError::config(format!("TOML error: {e}")))?;
    std::fs::write(&path, contents).map_err(DbtpError::Io)?;

    Ok(())
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
    let choice = prompt(&format!("{label}"), "s");
    if choice.eq_ignore_ascii_case("s") || choice.is_empty() {
        return None;
    }
    choice
        .parse::<usize>()
        .ok()
        .filter(|&n| n >= 1 && n <= items.len())
        .map(|n| items[n - 1].1.clone())
}
