use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Subcommand;
use reqwest::blocking::multipart;
use tracing::info;

use crate::client::WfClient;
use crate::config::Config;
use crate::models::{ImportParseResponse, ImportResponse};

#[derive(Debug, Subcommand)]
pub enum ActivityCmd {
    /// Parse a CSV server-side without writing anything. Useful as a dry-run.
    ImportParse {
        /// Target account id (server uses it to look up the account-specific mapping).
        #[arg(long)]
        account: String,
        /// Path to the CSV file.
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Validate a CSV import end-to-end (parse + check) without writing.
    ImportCheck {
        #[arg(long)]
        account: String,
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Upload + commit a CSV import. Writes activities to the account.
    Import {
        #[arg(long)]
        account: String,
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

pub fn run(cfg: &Config, cmd: &ActivityCmd) -> Result<()> {
    match cmd {
        ActivityCmd::ImportParse {
            account,
            file,
            json,
        } => upload(
            cfg,
            "/api/v1/activities/import/parse",
            account,
            file,
            *json,
            "parse",
        ),
        ActivityCmd::ImportCheck {
            account,
            file,
            json,
        } => upload(
            cfg,
            "/api/v1/activities/import/check",
            account,
            file,
            *json,
            "check",
        ),
        ActivityCmd::Import {
            account,
            file,
            json,
        } => upload(
            cfg,
            "/api/v1/activities/import",
            account,
            file,
            *json,
            "import",
        ),
    }
}

fn upload(
    cfg: &Config,
    endpoint: &str,
    account_id: &str,
    file: &PathBuf,
    emit_json: bool,
    label: &str,
) -> Result<()> {
    let client = WfClient::new(cfg)?;
    let form = multipart::Form::new()
        .text("accountId", account_id.to_string())
        .file("file", file)
        .with_context(|| format!("attach {}", file.display()))?;

    info!(endpoint, account_id, file = %file.display(), "uploading");

    match endpoint {
        "/api/v1/activities/import/parse" => {
            let resp: ImportParseResponse = client.post_multipart(endpoint, form)?;
            print_parse(&resp, emit_json)?;
        }
        _ => {
            let resp: ImportResponse = client.post_multipart(endpoint, form)?;
            if emit_json {
                println!("{}", serde_json::to_string_pretty(&resp)?);
            } else {
                println!("{label}: ok");
                println!("{}", serde_json::to_string_pretty(&resp)?);
            }
        }
    }
    Ok(())
}

fn print_parse(resp: &ImportParseResponse, emit_json: bool) -> Result<()> {
    if emit_json {
        println!("{}", serde_json::to_string_pretty(resp)?);
        return Ok(());
    }
    println!("rows detected : {}", resp.row_count);
    println!("headers       : {}", resp.headers.join(", "));
    println!("errors        : {}", resp.errors.len());
    if !resp.errors.is_empty() {
        for e in &resp.errors {
            println!("  - {e}");
        }
    }
    if !resp.rows.is_empty() {
        let preview = resp.rows.iter().take(3);
        println!("preview (first 3 rows):");
        for row in preview {
            println!("  {}", row.join(" | "));
        }
    }
    Ok(())
}
