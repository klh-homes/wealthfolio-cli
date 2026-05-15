//! HTTP wrapper. Builds a reqwest blocking Client pre-loaded with the cached
//! session cookie + base_url. Used by every command that hits the API.

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::de::DeserializeOwned;
use tracing::debug;

use crate::auth::{auth_headers, ensure_session};
use crate::config::Config;
use crate::error::WfError;

pub struct WfClient<'a> {
    pub cfg: &'a Config,
    inner: Client,
}

impl<'a> WfClient<'a> {
    /// Build a client with the cached session injected as default cookie header.
    /// Auto-logins (or refreshes) if necessary.
    pub fn new(cfg: &'a Config) -> Result<Self> {
        let session = ensure_session(cfg)?;
        let inner = Client::builder()
            .default_headers(auth_headers(&session)?)
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self { cfg, inner })
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.cfg.base_url, path)
    }

    /// GET <path> → deserialize JSON.
    pub fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = self.url(path);
        debug!(%url, "GET");
        let resp = self.inner.get(&url).send()?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(WfError::Http {
                status: status.as_u16(),
                body,
            }
            .into());
        }
        resp.json::<T>().context("parse JSON response")
    }

    /// POST JSON body, expect JSON response.
    pub fn post_json<B: serde::Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = self.url(path);
        debug!(%url, "POST");
        let resp = self.inner.post(&url).json(body).send()?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().unwrap_or_default();
            return Err(WfError::Http {
                status: status.as_u16(),
                body: text,
            }
            .into());
        }
        resp.json::<T>().context("parse JSON response")
    }

    /// DELETE <path>. Returns nothing on 2xx.
    pub fn delete(&self, path: &str) -> Result<()> {
        let url = self.url(path);
        debug!(%url, "DELETE");
        let resp = self.inner.delete(&url).send()?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(WfError::Http {
                status: status.as_u16(),
                body,
            }
            .into());
        }
        Ok(())
    }

    /// Multipart POST (for CSV import endpoints). Caller builds the form.
    pub fn post_multipart<T: DeserializeOwned>(
        &self,
        path: &str,
        form: reqwest::blocking::multipart::Form,
    ) -> Result<T> {
        let url = self.url(path);
        debug!(%url, "POST (multipart)");
        let resp = self.inner.post(&url).multipart(form).send()?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(WfError::Http {
                status: status.as_u16(),
                body,
            }
            .into());
        }
        resp.json::<T>().context("parse JSON response")
    }
}
