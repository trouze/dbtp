use std::io::{BufWriter, Write};
use std::process::{Command, Stdio};

use chrono::{DateTime, Utc};
use clap::{Args, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::Value;

use crate::api::admin::audit_logs;
use crate::cli::output::{format_output, OutputFormat};
use crate::core::config::Config;
use crate::core::error::{DbtpError, Result};
use crate::core::rest_client::RestClient;

#[derive(Debug, Args)]
pub struct AuditLogsArgs {
    #[command(subcommand)]
    pub command: AuditLogsCommand,
}

#[derive(Debug, Subcommand)]
pub enum AuditLogsCommand {
    /// List recent audit log events
    #[command(after_long_help = "EXAMPLES:\n  \
            dbtp audit-logs list\n  \
            dbtp audit-logs list -o json\n  \
            dbtp audit-logs list --include-related actor")]
    List {
        /// Comma-separated related objects to include (e.g. actor)
        #[arg(long)]
        include_related: Option<String>,
    },

    /// Export audit log events to stdout, a local file, or cloud storage
    ///
    /// Output is NDJSON (newline-delimited JSON) — one event per line.
    /// Compatible with Datadog, Azure Monitor, BigQuery, Splunk, and similar systems.
    ///
    /// Destinations:
    ///   -                              stdout (default, pipeable)
    ///   /path/to/file.ndjson           local file
    ///   gs://bucket/prefix/            Google Cloud Storage  (requires gsutil)
    ///   s3://bucket/prefix/            Amazon S3             (requires aws CLI)
    ///   az://account/container/path/   Azure Blob Storage    (requires az CLI)
    ///
    /// When the destination ends with '/', a filename is auto-generated:
    ///   audit-logs-<since-date>.ndjson
    ///
    /// Cloud CLI tools must be installed and authenticated independently —
    /// dbtp never handles cloud credentials.
    #[command(after_long_help = "EXAMPLES:\n  \
            # stdout (pipe to any tool)\n  \
            dbtp audit-logs export --since 2026-01-01T00:00:00Z\n\n  \
            # local file\n  \
            dbtp audit-logs export --since 2026-01-01T00:00:00Z --output ./audit-logs.ndjson\n\n  \
            # Google Cloud Storage (gsutil must be installed and authed)\n  \
            dbtp audit-logs export --since 2026-01-01T00:00:00Z --output gs://my-bucket/audit-logs/\n\n  \
            # Amazon S3 (aws CLI must be installed and authed)\n  \
            dbtp audit-logs export --since 2026-01-01T00:00:00Z --output s3://my-bucket/audit-logs/\n\n  \
            # Azure Blob Storage (az CLI must be installed and authed)\n  \
            # format: az://storage-account/container/blob-path\n  \
            dbtp audit-logs export --since 2026-01-01T00:00:00Z --output az://myaccount/mylogs/audit-logs/")]
    Export {
        /// Export events at or after this timestamp (RFC 3339, e.g. 2026-01-01T00:00:00Z)
        #[arg(long, required = true)]
        since: String,

        /// Where to write NDJSON output (default: stdout)
        #[arg(long, default_value = "-")]
        output: String,
    },
}

pub async fn exec(args: &AuditLogsArgs, client: &RestClient, config: &Config) -> Result<()> {
    match &args.command {
        AuditLogsCommand::List { include_related } => {
            let mut params = Vec::new();
            if let Some(rel) = include_related {
                params.push(("include_related".to_string(), rel.clone()));
            }
            let events = audit_logs::list(client, &params).await?;
            let val = Value::Array(events);
            let output_format = OutputFormat::parse(&config.output);
            println!("{}", format_output(&val, output_format));
        }

        AuditLogsCommand::Export { since, output } => {
            let since_dt = since.parse::<DateTime<Utc>>().map_err(|e| {
                DbtpError::config(format!(
                    "invalid --since '{}': {}. Use RFC 3339, e.g. 2026-01-01T00:00:00Z",
                    since, e
                ))
            })?;

            let dest = resolve_destination(output, &since_dt);

            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.cyan} {msg}")
                    .unwrap(),
            );
            pb.set_message("Fetching audit logs...");
            pb.enable_steady_tick(std::time::Duration::from_millis(100));

            let events = audit_logs::export_since(client, &since_dt).await?;
            let count = events.len();

            pb.finish_and_clear();

            write_ndjson(&events, &dest)?;

            eprintln!("Exported {count} audit log event(s) → {dest}");
        }
    }
    Ok(())
}

/// Expand a destination that ends with '/' into a full path with auto-generated filename.
fn resolve_destination(output: &str, since: &DateTime<Utc>) -> String {
    if output.ends_with('/') {
        let date = since.format("%Y-%m-%d");
        format!("{output}audit-logs-{date}.ndjson")
    } else {
        output.to_string()
    }
}

fn write_ndjson(events: &[Value], destination: &str) -> Result<()> {
    match parse_dest(destination) {
        Dest::Stdout => {
            let stdout = std::io::stdout();
            let mut w = BufWriter::new(stdout.lock());
            write_lines(&mut w, events)?;
        }
        Dest::LocalFile(path) => {
            let f = std::fs::File::create(&path)
                .map_err(|e| DbtpError::config(format!("cannot create '{}': {}", path, e)))?;
            let mut w = BufWriter::new(f);
            write_lines(&mut w, events)?;
        }
        Dest::Gcs(url) => {
            pipe_to_cmd(events, "gsutil", &["cp", "-", &url])?;
        }
        Dest::S3(url) => {
            pipe_to_cmd(events, "aws", &["s3", "cp", "-", &url])?;
        }
        Dest::Azure {
            account,
            container,
            blob,
        } => {
            pipe_to_cmd(
                events,
                "az",
                &[
                    "storage",
                    "blob",
                    "upload",
                    "--file",
                    "-",
                    "--account-name",
                    &account,
                    "--container-name",
                    &container,
                    "--name",
                    &blob,
                    "--overwrite",
                ],
            )?;
        }
    }
    Ok(())
}

fn write_lines<W: Write>(w: &mut W, events: &[Value]) -> Result<()> {
    for event in events {
        let line = serde_json::to_string(event)?;
        w.write_all(line.as_bytes())?;
        w.write_all(b"\n")?;
    }
    w.flush()?;
    Ok(())
}

fn pipe_to_cmd(events: &[Value], program: &str, args: &[&str]) -> Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| {
            DbtpError::config(format!(
                "failed to spawn '{}': {}. Is it installed and on PATH?",
                program, e
            ))
        })?;

    {
        let stdin = child.stdin.as_mut().expect("stdin was piped");
        let mut w = BufWriter::new(stdin);
        write_lines(&mut w, events)?;
    }

    let status = child.wait()?;
    if !status.success() {
        return Err(DbtpError::config(format!(
            "'{}' exited with status {}",
            program,
            status.code().unwrap_or(-1)
        )));
    }
    Ok(())
}

enum Dest {
    Stdout,
    LocalFile(String),
    Gcs(String),
    S3(String),
    Azure {
        account: String,
        container: String,
        blob: String,
    },
}

fn parse_dest(dest: &str) -> Dest {
    if dest == "-" {
        return Dest::Stdout;
    }
    if dest.starts_with("gs://") {
        return Dest::Gcs(dest.to_string());
    }
    if dest.starts_with("s3://") {
        return Dest::S3(dest.to_string());
    }
    // az://account/container/blob-path
    if let Some(rest) = dest.strip_prefix("az://") {
        let parts: Vec<&str> = rest.splitn(3, '/').collect();
        if parts.len() == 3 {
            return Dest::Azure {
                account: parts[0].to_string(),
                container: parts[1].to_string(),
                blob: parts[2].to_string(),
            };
        }
    }
    Dest::LocalFile(dest.to_string())
}
