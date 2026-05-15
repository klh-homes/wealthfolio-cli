use thiserror::Error;

#[derive(Debug, Error)]
pub enum WfError {
    #[error("WEALTHFOLIO_PASSWORD not set; cannot login")]
    PasswordMissing,
    #[error("login failed (HTTP {0}); check WEALTHFOLIO_PASSWORD")]
    LoginFailed(u16),
    #[error("server returned {status}: {body}")]
    Http { status: u16, body: String },
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
