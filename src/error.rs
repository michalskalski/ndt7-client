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
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),
    #[error("bad service URL: {0}")]
    ServiceUnsupported(#[from] url::ParseError),
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

// reducing size of Ndt7error by putting large element in the Box
impl From<tokio_tungstenite::tungstenite::Error> for Ndt7Error {
    fn from(e: tokio_tungstenite::tungstenite::Error) -> Self {
        Ndt7Error::WebSocket(Box::new(e))
    }
}

pub type Result<T> = std::result::Result<T, Ndt7Error>;
