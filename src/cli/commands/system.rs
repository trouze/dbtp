use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use serde::Deserialize;

use crate::core::config;
use crate::core::error::{DbtpError, Result};

const REPO: &str = "trouze/dbtp";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Args)]
pub struct SystemArgs {
    #[command(subcommand)]
    pub command: SystemCommand,
}

#[derive(Debug, Subcommand)]
pub enum SystemCommand {
    /// Show dbtp version, install path, and config location
    Info,
    /// Update dbtp to the latest release from GitHub
    Update {
        /// Specific version to install (e.g. "v0.2.0"). Defaults to latest.
        #[arg(long)]
        version: Option<String>,
    },
    /// Uninstall dbtp binary and optionally remove config
    Uninstall {
        /// Also remove the config directory (~/.config/dbtp)
        #[arg(long)]
        purge: bool,
    },
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

fn detect_target() -> Result<&'static str> {
    let target = match (env::consts::OS, env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        (os, arch) => {
            return Err(DbtpError::config(format!(
                "Unsupported platform: {os}/{arch}"
            )));
        }
    };
    Ok(target)
}

fn binary_path() -> Result<PathBuf> {
    env::current_exe().map_err(DbtpError::Io)
}

pub async fn exec(args: &SystemArgs) -> Result<()> {
    match &args.command {
        SystemCommand::Info => info().await,
        SystemCommand::Update { version } => update(version.as_deref()).await,
        SystemCommand::Uninstall { purge } => uninstall(*purge),
    }
}

async fn info() -> Result<()> {
    let exe = binary_path().ok();
    let config_dir = config::config_dir().ok();

    eprintln!("dbtp {CURRENT_VERSION}");
    eprintln!();
    if let Some(path) = &exe {
        eprintln!("Binary:  {}", path.display());
    }
    if let Some(dir) = &config_dir {
        let exists = dir.exists();
        eprintln!(
            "Config:  {}{}",
            dir.display(),
            if exists { "" } else { " (not created yet)" }
        );
    }
    eprintln!("Target:  {}", detect_target().unwrap_or("unknown"));
    eprintln!("Repo:    https://github.com/{REPO}");

    Ok(())
}

async fn update(pin_version: Option<&str>) -> Result<()> {
    let target = detect_target()?;
    let exe_path = binary_path()?;

    eprintln!("Current version: v{CURRENT_VERSION}");

    let client = reqwest::Client::builder()
        .user_agent("dbtp-updater")
        .build()
        .map_err(DbtpError::Http)?;

    let release: GithubRelease = if let Some(v) = pin_version {
        let tag = if v.starts_with('v') {
            v.to_string()
        } else {
            format!("v{v}")
        };
        let url = format!("https://api.github.com/repos/{REPO}/releases/tags/{tag}");
        client
            .get(&url)
            .send()
            .await
            .map_err(DbtpError::Http)?
            .json()
            .await
            .map_err(DbtpError::Http)?
    } else {
        let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
        client
            .get(&url)
            .send()
            .await
            .map_err(DbtpError::Http)?
            .json()
            .await
            .map_err(DbtpError::Http)?
    };

    let latest = &release.tag_name;
    let current_tag = format!("v{CURRENT_VERSION}");

    if latest == &current_tag && pin_version.is_none() {
        eprintln!("Already up to date ({latest}).");
        return Ok(());
    }

    eprintln!("Updating to {latest}...");

    let expected_name = format!("dbtp-{latest}-{target}.tar.gz");
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == expected_name)
        .ok_or_else(|| {
            DbtpError::config(format!(
                "No binary found for {target} in release {latest}. Available: {}",
                release
                    .assets
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;

    let bytes = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .map_err(DbtpError::Http)?
        .bytes()
        .await
        .map_err(DbtpError::Http)?;

    let tmpdir = tempdir(&exe_path)?;
    let tarball = tmpdir.join("dbtp.tar.gz");
    fs::write(&tarball, &bytes).map_err(DbtpError::Io)?;

    let status = std::process::Command::new("tar")
        .args([
            "xzf",
            &tarball.to_string_lossy(),
            "-C",
            &tmpdir.to_string_lossy(),
        ])
        .status()
        .map_err(DbtpError::Io)?;

    if !status.success() {
        return Err(DbtpError::config("Failed to extract update archive"));
    }

    let new_binary = tmpdir.join("dbtp");
    if !new_binary.exists() {
        return Err(DbtpError::config(
            "Extracted archive does not contain dbtp binary",
        ));
    }

    let backup = exe_path.with_extension("old");
    if backup.exists() {
        fs::remove_file(&backup).ok();
    }
    fs::rename(&exe_path, &backup).map_err(DbtpError::Io)?;

    if let Err(e) = fs::rename(&new_binary, &exe_path) {
        fs::rename(&backup, &exe_path).ok();
        return Err(DbtpError::Io(e));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&exe_path, fs::Permissions::from_mode(0o755)).map_err(DbtpError::Io)?;
    }

    fs::remove_file(&backup).ok();
    fs::remove_dir_all(&tmpdir).ok();

    eprintln!("Updated dbtp to {latest}.");
    Ok(())
}

fn uninstall(purge: bool) -> Result<()> {
    let exe_path = binary_path()?;

    if purge {
        if let Ok(dir) = config::config_dir() {
            if dir.exists() {
                fs::remove_dir_all(&dir).map_err(DbtpError::Io)?;
                eprintln!("Removed config directory: {}", dir.display());
            }
        }
    }

    eprintln!("Removing binary: {}", exe_path.display());
    fs::remove_file(&exe_path).map_err(DbtpError::Io)?;
    eprintln!("dbtp has been uninstalled.");

    if !purge {
        if let Ok(dir) = config::config_dir() {
            if dir.exists() {
                eprintln!(
                    "\nConfig directory was kept at {}. Use --purge to remove it.",
                    dir.display()
                );
            }
        }
    }

    Ok(())
}

fn tempdir(near: &Path) -> Result<PathBuf> {
    let dir = near
        .parent()
        .unwrap_or(std::path::Path::new("/tmp"))
        .join(".dbtp-update");
    if dir.exists() {
        fs::remove_dir_all(&dir).ok();
    }
    fs::create_dir_all(&dir).map_err(DbtpError::Io)?;
    Ok(dir)
}
