//! Error types for the ndt7 client.

use crate::client::AddressFamily;
use thiserror::Error;

/// Errors that can occur during ndt7 operations.
#[derive(Debug, Error)]
pub enum Ndt7Error {
    /// The Locate API HTTP request failed.
    #[error("locate failed: {0}")]
    LocateFailed(#[from] reqwest::Error),
    /// The Locate API returned no test targets.
    #[error("no targets available")]
    NoTargets,
    /// The Locate API returned 204: M-Lab is out of capacity.
    #[error("server at capacity; try again later")]
    NoCapacity,
    /// JSON serialization or deserialization failed.
    #[error("serialize/deserialize error: {0}")]
    JsonError(#[from] serde_json::Error),
    /// A test exceeded its time limit.
    #[error("timeout occured")]
    Timeout(#[from] tokio::time::error::Elapsed),
    /// A WebSocket-level error occurred.
    #[error("websocket error: {0}")]
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),
    /// The provided service URL path is not a recognized ndt7 endpoint.
    #[error("bad service URL: {0}")]
    ServiceUnsupported(String),
    /// The URL could not be parsed.
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),
    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    /// Protocol violation
    #[error("protocol violation: {0}")]
    ProtocolViolation(String),
    /// No addresses of the requested IP family were found for the host.
    #[error("no {0} address found")]
    NoAddressFound(AddressFamily),
}

// Reducing size of Ndt7Error by boxing the large tungstenite::Error variant.
impl From<tokio_tungstenite::tungstenite::Error> for Ndt7Error {
    fn from(e: tokio_tungstenite::tungstenite::Error) -> Self {
        Ndt7Error::WebSocket(Box::new(e))
    }
}

/// A `Result` type alias using [`Ndt7Error`].
pub type Result<T> = std::result::Result<T, Ndt7Error>;
