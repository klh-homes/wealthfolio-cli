use anyhow::Result;
use clap::Subcommand;

use crate::client::WfClient;
use crate::config::Config;
use crate::models::{Account, AccountUpdate, NewAccount};

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
        #[arg(long, default_value = "TRANSACTIONS")]
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
    /// Update an account. Fetches the current record, applies the given
    /// changes, and PUTs the result. Only the flags you pass are
    /// changed.
    Update {
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        currency: Option<String>,
        #[arg(long = "type")]
        account_type: Option<String>,
        /// HOLDINGS or TRANSACTIONS.
        #[arg(long)]
        tracking: Option<String>,
        #[arg(long)]
        group: Option<String>,
        #[arg(long)]
        platform_id: Option<String>,
        #[arg(long)]
        account_number: Option<String>,
        #[arg(long)]
        default: Option<bool>,
        #[arg(long)]
        active: Option<bool>,
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
        AccountCmd::Update {
            id,
            name,
            currency,
            account_type,
            tracking,
            group,
            platform_id,
            account_number,
            default,
            active,
            json,
        } => update(
            cfg,
            UpdateArgs {
                id: id.clone(),
                name: name.clone(),
                currency: currency.clone(),
                account_type: account_type.clone(),
                tracking: tracking.clone(),
                group: group.clone(),
                platform_id: platform_id.clone(),
                account_number: account_number.clone(),
                is_default: *default,
                is_active: *active,
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
    let account = fetch_account(&client, id)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&account)?);
    } else {
        println!("{:#?}", account);
    }
    Ok(())
}

fn fetch_account(client: &WfClient, id: &str) -> Result<Account> {
    // The list endpoint is the only documented account-read endpoint;
    // grab the one we want from the full list. Fast enough for personal use.
    let accounts: Vec<Account> = client.get_json("/api/v1/accounts")?;
    accounts
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| anyhow::anyhow!("no account with id {id}"))
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

struct UpdateArgs {
    id: String,
    name: Option<String>,
    currency: Option<String>,
    account_type: Option<String>,
    tracking: Option<String>,
    group: Option<String>,
    platform_id: Option<String>,
    account_number: Option<String>,
    is_default: Option<bool>,
    is_active: Option<bool>,
    emit_json: bool,
}

fn update(cfg: &Config, args: UpdateArgs) -> Result<()> {
    let client = WfClient::new(cfg)?;
    let current = fetch_account(&client, &args.id)?;
    let mut body = AccountUpdate::from(&current);
    if let Some(v) = args.name {
        body.name = v;
    }
    if let Some(v) = args.currency {
        body.currency = v;
    }
    if let Some(v) = args.account_type {
        body.account_type = v;
    }
    if let Some(v) = args.tracking {
        body.tracking_mode = v;
    }
    if let Some(v) = args.group {
        body.group = Some(v);
    }
    if let Some(v) = args.platform_id {
        body.platform_id = Some(v);
    }
    if let Some(v) = args.account_number {
        body.account_number = Some(v);
    }
    if let Some(v) = args.is_default {
        body.is_default = v;
    }
    if let Some(v) = args.is_active {
        body.is_active = v;
    }
    let updated: Account = client.put_json(&format!("/api/v1/accounts/{}", args.id), &body)?;
    if args.emit_json {
        println!("{}", serde_json::to_string_pretty(&updated)?);
    } else {
        println!(
            "updated: {} ({}) — tracking={} currency={}",
            updated.name, updated.id, updated.tracking_mode, updated.currency
        );
    }
    Ok(())
}

fn delete(cfg: &Config, id: &str) -> Result<()> {
    let client = WfClient::new(cfg)?;
    client.delete(&format!("/api/v1/accounts/{id}"))?;
    println!("deleted: {id}");
    Ok(())
}
