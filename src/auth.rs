//! Cookie-based session auth for Wealthfolio.
//!
//! Server sets `wf_session` cookie (JWT inside) with `Max-Age=3600`,
//! `HttpOnly`, `Secure`, `Path=/api`. We persist just the cookie value +
//! computed expiry between CLI invocations under `<cache>/cookies.json`
//! and inject it on every request via the `cookie:` header.

use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, SET_COOKIE};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::config::Config;
use crate::error::WfError;

const SESSION_COOKIE_NAME: &str = "wf_session";
/// Refresh when this much time remains (i.e. ~50 % of TTL elapsed).
const REFRESH_REMAINING: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCache {
    pub session: String,
    pub expires_at: DateTime<Utc>,
}

impl SessionCache {
    pub fn fresh_enough(&self) -> bool {
        let now = Utc::now();
        let remaining = self.expires_at - now;
        remaining > chrono::Duration::from_std(REFRESH_REMAINING).unwrap_or_default()
    }

    fn load(path: &Path) -> Option<Self> {
        let data = fs::read(path).ok()?;
        serde_json::from_slice(&data).ok()
    }

    fn save(&self, path: &Path) -> Result<()> {
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        fs::rename(&tmp, path)?;
        // best-effort: tighten perms (errors ignored on platforms without unix perms)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

/// Perform `POST /api/v1/auth/login` and return a freshly cached session.
pub fn login(cfg: &Config) -> Result<SessionCache> {
    let password = cfg.password.as_deref().ok_or(WfError::PasswordMissing)?;

    let url = format!("{}/api/v1/auth/login", cfg.base_url);
    debug!(%url, "logging in");

    // The login endpoint sets a cookie; we need to read Set-Cookie ourselves
    // (reqwest's cookie store would also work, but we already need to extract
    // Max-Age to compute expiry, so manual parsing keeps the path single).
    let resp = Client::builder()
        .build()?
        .post(&url)
        .json(&serde_json::json!({ "password": password }))
        .send()?;

    let status = resp.status();
    if !status.is_success() {
        return Err(WfError::LoginFailed(status.as_u16()).into());
    }

    let set_cookie = resp
        .headers()
        .get(SET_COOKIE)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| anyhow!("login response missing Set-Cookie header"))?;

    let (value, max_age) = parse_session_cookie(set_cookie)?;
    let expires_at = Utc::now() + chrono::Duration::seconds(max_age as i64);

    let cache = SessionCache {
        session: value,
        expires_at,
    };
    cache.save(&cfg.cookies_path())?;
    info!(?expires_at, "logged in");
    Ok(cache)
}

/// Read cached session; refresh by re-login when missing/expiring soon.
pub fn ensure_session(cfg: &Config) -> Result<SessionCache> {
    match SessionCache::load(&cfg.cookies_path()) {
        Some(c) if c.fresh_enough() => {
            debug!(expires_at=?c.expires_at, "session cache hit");
            Ok(c)
        }
        Some(_) => {
            debug!("session cached but stale; re-logging in");
            login(cfg)
        }
        None => {
            debug!("no session cached; logging in");
            login(cfg)
        }
    }
}

/// Forget the cached session.
pub fn logout(cfg: &Config) -> Result<()> {
    let path = cfg.cookies_path();
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}

/// Headers to send with an authenticated request.
pub fn auth_headers(cache: &SessionCache) -> Result<HeaderMap> {
    let mut h = HeaderMap::new();
    let val = format!("{}={}", SESSION_COOKIE_NAME, cache.session);
    h.insert(COOKIE, HeaderValue::from_str(&val)?);
    Ok(h)
}

/// Parse `wf_session=eyJ...; Max-Age=3600; ...` into (value, max_age_secs).
fn parse_session_cookie(set_cookie: &str) -> Result<(String, u64)> {
    let mut value: Option<String> = None;
    let mut max_age: u64 = 3600; // sensible default if Max-Age missing
    for part in set_cookie.split(';') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix(&format!("{SESSION_COOKIE_NAME}=")) {
            value = Some(v.to_string());
        } else if let Some(v) = part.strip_prefix("Max-Age=") {
            if let Ok(n) = v.parse::<u64>() {
                max_age = n;
            }
        }
    }
    Ok((
        value.ok_or_else(|| anyhow!("Set-Cookie has no {SESSION_COOKIE_NAME} key"))?,
        max_age,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_set_cookie_from_live_server() {
        // Captured from a real Wealthfolio v3.4.0 login response.
        let h = "wf_session=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.payload.sig; \
                 HttpOnly; SameSite=Lax; Path=/api; Max-Age=3600; Secure";
        let (value, max_age) = parse_session_cookie(h).unwrap();
        assert_eq!(max_age, 3600);
        assert!(value.starts_with("eyJ"));
        assert!(value.ends_with(".sig"));
    }

    #[test]
    fn defaults_max_age_when_absent() {
        let h = "wf_session=abc; HttpOnly";
        let (value, max_age) = parse_session_cookie(h).unwrap();
        assert_eq!(value, "abc");
        assert_eq!(max_age, 3600);
    }

    #[test]
    fn errors_when_session_cookie_missing() {
        let h = "other_cookie=foo; Max-Age=3600";
        assert!(parse_session_cookie(h).is_err());
    }

    #[test]
    fn fresh_enough_uses_refresh_window() {
        let near_expiry = SessionCache {
            session: "x".into(),
            expires_at: Utc::now() + chrono::Duration::minutes(10),
        };
        assert!(!near_expiry.fresh_enough());

        let plenty = SessionCache {
            session: "x".into(),
            expires_at: Utc::now() + chrono::Duration::minutes(45),
        };
        assert!(plenty.fresh_enough());
    }
}
