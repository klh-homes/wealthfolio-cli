use anyhow::Result;
use clap::Subcommand;

use crate::client::WfClient;
use crate::config::Config;
use crate::models::NetWorth;

#[derive(Debug, Subcommand)]
pub enum NetWorthCmd {
    /// Current net worth snapshot.
    Current {
        #[arg(long)]
        json: bool,
    },
}

pub fn run(cfg: &Config, cmd: &NetWorthCmd) -> Result<()> {
    match cmd {
        NetWorthCmd::Current { json } => current(cfg, *json),
    }
}

fn current(cfg: &Config, json: bool) -> Result<()> {
    let client = WfClient::new(cfg)?;
    let nw: NetWorth = client.get_json("/api/v1/net-worth")?;
    if json {
        println!("{}", serde_json::to_string_pretty(&nw)?);
        return Ok(());
    }
    println!("date         {}", nw.date);
    println!("currency     {}", nw.currency);
    println!("net worth    {:>16.2}", nw.net_worth);
    println!("  assets     {:>16.2}", nw.assets.total);
    println!("  liabilities{:>16.2}", nw.liabilities.total);
    Ok(())
}
