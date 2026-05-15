use anyhow::Result;
use clap::Subcommand;

use crate::client::WfClient;
use crate::config::Config;
use crate::models::{Account, NewAccount};

#[derive(Debug, Subcommand)]
pub enum AccountCmd {
    /// List all accounts.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Get one account by id.
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Create a new account.
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        currency: String,
        /// Account type. Common values: SAVINGS, BROKERAGE, CRYPTO, CASH, ASSET.
        #[arg(long = "type")]
        account_type: String,
        /// HOLDINGS (value snapshots) or TRANSACTIONS (full activity ledger).
        #[arg(long, default_value = "HOLDINGS")]
        tracking: String,
        #[arg(long)]
        default: bool,
        #[arg(long)]
        platform_id: Option<String>,
        #[arg(long)]
        group: Option<String>,
        #[arg(long)]
        account_number: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Delete an account by id.
    Delete { id: String },
}

pub fn run(cfg: &Config, cmd: &AccountCmd) -> Result<()> {
    match cmd {
        AccountCmd::List { json } => list(cfg, *json),
        AccountCmd::Get { id, json } => get(cfg, id, *json),
        AccountCmd::Create {
            name,
            currency,
            account_type,
            tracking,
            default,
            platform_id,
            group,
            account_number,
            json,
        } => create(
            cfg,
            CreateArgs {
                name: name.clone(),
                currency: currency.clone(),
                account_type: account_type.clone(),
                tracking: tracking.clone(),
                is_default: *default,
                platform_id: platform_id.clone(),
                group: group.clone(),
                account_number: account_number.clone(),
                emit_json: *json,
            },
        ),
        AccountCmd::Delete { id } => delete(cfg, id),
    }
}

fn list(cfg: &Config, json: bool) -> Result<()> {
    let client = WfClient::new(cfg)?;
    let accounts: Vec<Account> = client.get_json("/api/v1/accounts")?;
    if json {
        println!("{}", serde_json::to_string_pretty(&accounts)?);
        return Ok(());
    }
    if accounts.is_empty() {
        println!("(no accounts)");
        return Ok(());
    }
    println!(
        "{:<36}  {:<10}  {:<14}  {:<12}  NAME",
        "ID", "CURRENCY", "TYPE", "TRACKING"
    );
    for a in &accounts {
        println!(
            "{:<36}  {:<10}  {:<14}  {:<12}  {}",
            a.id, a.currency, a.account_type, a.tracking_mode, a.name
        );
    }
    Ok(())
}

fn get(cfg: &Config, id: &str, json: bool) -> Result<()> {
    let client = WfClient::new(cfg)?;
    // The list endpoint is the only documented account read endpoint;
    // grab the one we want from the full list. Fast enough for small instances.
    let accounts: Vec<Account> = client.get_json("/api/v1/accounts")?;
    let account = accounts
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| anyhow::anyhow!("no account with id {id}"))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&account)?);
    } else {
        println!("{:#?}", account);
    }
    Ok(())
}

struct CreateArgs {
    name: String,
    currency: String,
    account_type: String,
    tracking: String,
    is_default: bool,
    platform_id: Option<String>,
    group: Option<String>,
    account_number: Option<String>,
    emit_json: bool,
}

fn create(cfg: &Config, args: CreateArgs) -> Result<()> {
    let client = WfClient::new(cfg)?;
    let body = NewAccount {
        name: args.name,
        account_type: args.account_type,
        currency: args.currency,
        is_default: args.is_default,
        is_active: true,
        tracking_mode: args.tracking,
        group: args.group,
        platform_id: args.platform_id,
        account_number: args.account_number,
    };
    let created: Account = client.post_json("/api/v1/accounts", &body)?;
    if args.emit_json {
        println!("{}", serde_json::to_string_pretty(&created)?);
    } else {
        println!("created: {} ({})", created.name, created.id);
    }
    Ok(())
}

fn delete(cfg: &Config, id: &str) -> Result<()> {
    let client = WfClient::new(cfg)?;
    client.delete(&format!("/api/v1/accounts/{id}"))?;
    println!("deleted: {id}");
    Ok(())
}
