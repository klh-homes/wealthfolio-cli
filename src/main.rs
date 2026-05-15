use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod auth;
mod client;
mod commands;
mod config;
mod error;
mod models;

use commands::Cmd;
use config::Config;

#[derive(Debug, Parser)]
#[command(
    name = "wf",
    version,
    about = "Rust CLI for Wealthfolio's REST API",
    propagate_version = true
)]
struct Args {
    /// Increase log verbosity (-v=debug, -vv=trace). Default warn.
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    cmd: Cmd,
}

fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing(args.verbose);

    let cfg = Config::from_env()?;

    match &args.cmd {
        Cmd::Login => commands::login::run(&cfg),
        Cmd::Logout => commands::login::logout(&cfg),
        Cmd::Doctor => commands::doctor::run(&cfg),
        Cmd::Accounts(c) => commands::accounts::run(&cfg, c),
        Cmd::Activities(c) => commands::activities::run(&cfg, c),
        Cmd::NetWorth(c) => commands::net_worth::run(&cfg, c),
    }
}

fn init_tracing(verbose: u8) {
    let default = match verbose {
        0 => "warn",
        1 => "info,wealthfolio_cli=debug",
        2 => "debug",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();
}
