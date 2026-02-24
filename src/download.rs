//! ndt7 download test implementation.
//!
//! Receives binary and text WebSocket messages from the server until the
//! connection closes or [`params::DOWNLOAD_TIMEOUT`] elapses.

use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio::time::{Instant, timeout};
use tokio_tungstenite::tungstenite::Message;

use crate::client::WsStream;
use crate::error::Result;
use crate::params;
use crate::spec::{AppInfo, Measurement, Origin, TestKind};

/// Run the download test on an established WebSocket connection.
///
/// Measurements are sent on `tx` as they arrive. The function returns when
/// the server closes the connection or the timeout expires.
pub async fn run(mut ws: WsStream, tx: mpsc::Sender<Measurement>) -> Result<()> {
    let result = timeout(params::DOWNLOAD_TIMEOUT, download_loop(&mut ws, &tx)).await;

    // timeout returns Ok(inner_result) or Err(Elapsed)
    match result {
        Ok(inner) => inner,      // propagate any WebSocket error
        Err(_elapsed) => Ok(()), // timeout is normal completion
    }
}

async fn download_loop(ws: &mut WsStream, tx: &mpsc::Sender<Measurement>) -> Result<()> {
    let start = Instant::now();
    let mut prev_update = start;
    let mut total_bytes: i64 = 0;

    while let Some(msg) = ws.next().await {
        let msg = msg?;
        match msg {
            Message::Binary(data) => {
                total_bytes += data.len() as i64;
            }
            Message::Text(text) => {
                let mut measurement: Measurement = serde_json::from_str(&text)?;
                measurement.origin = Some(Origin::Server);
                measurement.test = Some(TestKind::Download);
                let _ = tx.send(measurement).await;
                total_bytes += text.len() as i64;
            }
            Message::Close(_) => break,
            _ => {} // Ping/Pong handled automatically by tokio-tungstenite
        }
        if prev_update.elapsed() >= params::UPDATE_INTERVAL {
            prev_update = Instant::now();
            let _ = tx
                .send(Measurement {
                    app_info: Some(AppInfo {
                        elapsed_time: start.elapsed().as_micros() as i64,
                        num_bytes: total_bytes,
                    }),
                    origin: Some(Origin::Client),
                    test: Some(TestKind::Download),
                    ..Default::default()
                })
                .await;
        }
    }
    Ok(())
}
