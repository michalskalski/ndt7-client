use thiserror::Error;

#[derive(Debug, Error)]
pub enum Ndt7Error {
    #[error("locate failed: {0}")]
    LocateFailed(#[from] reqwest::Error),
    #[error("no targets available")]
    NoTargets,
    #[error("serialize/deserialize error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("timeout occured")]
    Timeout,
    #[error("websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("bad service URL: {0}")]
    ServiceUnsupported(#[from] url::ParseError),
}

pub type Result<T> = std::result::Result<T, Ndt7Error>;
