pub mod accounts;
pub mod activities;
pub mod doctor;
pub mod login;
pub mod net_worth;

use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Force a fresh login and update the cached cookie.
    Login,
    /// Wipe the cached session cookie.
    Logout,
    /// End-to-end health check: env / DNS / login / cookie cache.
    Doctor,

    /// Account operations.
    #[command(subcommand)]
    Accounts(accounts::AccountCmd),

    /// Activity operations: CSV import, JSON bulk-create, search, delete.
    #[command(subcommand)]
    Activities(activities::ActivityCmd),

    /// Net-worth snapshots and history.
    #[command(subcommand, name = "net-worth")]
    NetWorth(net_worth::NetWorthCmd),
}
