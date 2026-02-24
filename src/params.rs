//! Protocol constants and tuning parameters.

use std::time::Duration;

/// Value of the Sec-WebSocket-Protocol header.
pub const SEC_WEBSOCKET_PROTOCOL: &str = "net.measurementlab.ndt.v7";

/// URL path for the download test.
pub const DOWNLOAD_URL_PATH: &str = "/ndt/v7/download";

/// URL path for the upload test.
pub const UPLOAD_URL_PATH: &str = "/ndt/v7/upload";

/// Initial size of uploaded messages (8 KiB).
pub const INITIAL_MESSAGE_SIZE: usize = 1 << 13;

/// Maximum accepted message size (1 MiB).
pub const MAX_MESSAGE_SIZE: usize = 1 << 20;

/// Threshold for scaling binary messages. When the current message size is
/// <= 1/SCALING_FRACTION of the total bytes sent, the message size doubles.
pub const SCALING_FRACTION: usize = 16;

/// Time after which the download test must stop.
pub const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15);

/// Time after which the upload test must stop.
pub const UPLOAD_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for individual I/O operations.
pub const IO_TIMEOUT: Duration = Duration::from_secs(7);

/// Interval between client-side measurement updates.
pub const UPDATE_INTERVAL: Duration = Duration::from_millis(250);
