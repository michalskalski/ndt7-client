//! An [ndt7](https://github.com/m-lab/ndt-server/blob/master/spec/ndt7-protocol.md) speed test
//! client library.
//!
//! ndt7 is a network performance measurement protocol developed by
//! [M-Lab](https://www.measurementlab.net/). It measures download and upload throughput
//! over WebSocket connections and reports TCP-level metrics such as latency and retransmission.
//!
//! # Quick start
//!
//! ```no_run
//! use ndt7_client::client::ClientBuilder;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let client = ClientBuilder::new("my-app", "0.1.0").build();
//! let targets = client.locate_test_targets().await?;
//!
//! if let Some(url) = &targets.download_url {
//!     let mut rx = client.start_download(url).await?;
//!     while let Some(m) = rx.recv().await {
//!         println!("{:?}", m);
//!     }
//! }
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

pub mod client;
pub mod download;
pub mod emitter;
pub mod error;
pub mod locate;
pub mod params;
pub mod spec;
pub mod summary;
pub mod upload;
