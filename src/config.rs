use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;

/// Runtime config sourced from env. CLI flags override at the call site.
#[derive(Debug, Clone)]
pub struct Config {
    pub base_url: String,
    pub password: Option<String>,
    pub cache_dir: PathBuf,
}

impl Config {
    /// Read env. `WEALTHFOLIO_BASE_URL` required; `WEALTHFOLIO_PASSWORD` optional
    /// at config time (some commands like `logout` don't need it).
    pub fn from_env() -> Result<Self> {
        let base_url = std::env::var("WEALTHFOLIO_BASE_URL")
            .context("WEALTHFOLIO_BASE_URL not set")?
            .trim_end_matches('/')
            .to_string();
        let password = std::env::var("WEALTHFOLIO_PASSWORD").ok();

        // ~/.cache/wf/ (XDG-compliant; on macOS uses ~/Library/Caches/wf/)
        let dirs = ProjectDirs::from("", "", "wf")
            .context("could not determine cache dir for current user")?;
        let cache_dir = dirs.cache_dir().to_path_buf();
        std::fs::create_dir_all(&cache_dir)
            .with_context(|| format!("create cache dir {}", cache_dir.display()))?;

        Ok(Self {
            base_url,
            password,
            cache_dir,
        })
    }

    pub fn cookies_path(&self) -> PathBuf {
        self.cache_dir.join("cookies.json")
    }
}
