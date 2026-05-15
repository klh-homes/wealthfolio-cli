use anyhow::Result;

use crate::auth;
use crate::config::Config;

pub fn run(cfg: &Config) -> Result<()> {
    let cache = auth::login(cfg)?;
    println!("logged in; session expires at {}", cache.expires_at);
    Ok(())
}

pub fn logout(cfg: &Config) -> Result<()> {
    auth::logout(cfg)?;
    println!("session cleared");
    Ok(())
}
