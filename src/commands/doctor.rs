//! Health check: prints the resolved env config, attempts a login, and
//! reports whether basic API access works. Designed to be the first thing
//! anyone runs when the pipeline misbehaves.

use anyhow::Result;
use reqwest::blocking::Client;

use crate::auth;
use crate::client::WfClient;
use crate::config::Config;

pub fn run(cfg: &Config) -> Result<()> {
    println!("=== wf doctor ===");
    println!("base_url            : {}", cfg.base_url);
    println!(
        "password            : {}",
        if cfg.password.is_some() {
            "<set>"
        } else {
            "<missing>"
        }
    );
    println!("cache_dir           : {}", cfg.cache_dir.display());
    println!("cookies_path        : {}", cfg.cookies_path().display());

    // 1. Plain HTTP reachability (no auth).
    print!("connectivity        : ");
    match Client::new().get(&cfg.base_url).send() {
        Ok(r) => println!("HTTP {} ✓", r.status().as_u16()),
        Err(e) => {
            println!("✗ {e}");
            return Ok(());
        }
    }

    // 2. Force a login round-trip (refreshes cache).
    print!("login               : ");
    match auth::login(cfg) {
        Ok(c) => println!("ok, expires {}", c.expires_at),
        Err(e) => {
            println!("✗ {e}");
            return Ok(());
        }
    }

    // 3. Authenticated GET — pick the cheapest endpoint.
    print!("authed call (/auth/me): ");
    let client = WfClient::new(cfg)?;
    match client.get_json::<serde_json::Value>("/api/v1/auth/me") {
        Ok(v) => println!("{} ✓", v),
        Err(e) => println!("✗ {e}"),
    }

    Ok(())
}
