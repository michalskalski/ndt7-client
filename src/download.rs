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
/// Measurements are sent on `tx` as they arrive. If a mid-test error
/// occurs (connection reset, malformed frame), it is sent as the final
/// item on the channel before it closes. The function returns when
/// the server closes the connection or the timeout expires.
pub async fn run(mut ws: WsStream, tx: mpsc::Sender<Result<Measurement>>) {
    let result = timeout(params::DOWNLOAD_TIMEOUT, download_loop(&mut ws, &tx)).await;

    // Overall timeout (Err) is normal completion, test ran its full duration.
    // Only errors from download_loop (Ok(Err)), like per-message IO timeouts,
    // are sent on the channel.
    if let Ok(Err(e)) = result {
        let _ = tx.send(Err(e)).await;
    }
}

async fn download_loop(ws: &mut WsStream, tx: &mpsc::Sender<Result<Measurement>>) -> Result<()> {
    let start = Instant::now();
    let mut prev_update = start;
    let mut total_bytes: i64 = 0;

    loop {
        let msg = timeout(params::IO_TIMEOUT, ws.next()).await?;
        let Some(msg) = msg else { break };
        let msg = msg?;
        match msg {
            Message::Binary(data) => {
                total_bytes += data.len() as i64;
            }
            Message::Text(text) => {
                let mut measurement: Measurement = serde_json::from_str(&text)?;
                measurement.origin = Some(Origin::Server);
                measurement.test = Some(TestKind::Download);
                let _ = tx.send(Ok(measurement)).await;
                total_bytes += text.len() as i64;
            }
            Message::Close(_) => break,
            _ => {} // Ping/Pong handled automatically by tokio-tungstenite
        }
        if prev_update.elapsed() >= params::UPDATE_INTERVAL {
            prev_update = Instant::now();
            let _ = tx
                .send(Ok(Measurement {
                    app_info: Some(AppInfo {
                        elapsed_time: start.elapsed().as_micros() as i64,
                        num_bytes: total_bytes,
                    }),
                    origin: Some(Origin::Client),
                    test: Some(TestKind::Download),
                    ..Default::default()
                }))
                .await;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    use futures_util::SinkExt;
    use tokio::net::TcpListener;

    use super::*;

    async fn mock_stalling_server() -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // server task in the background
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let ws_stream = tokio_tungstenite::accept_async(stream).await.unwrap();
            let (mut sink, _stream) = ws_stream.split();
            sink.send(Message::Text(
                r#"{"AppInfo":{"ElapsedTime":1000,"NumBytes":8192}}"#.into(),
            ))
            .await
            .unwrap();

            futures_util::future::pending::<()>().await;
        });
        addr
    }

    #[tokio::test(start_paused = true)]
    async fn test_mid_test_io_timeout() {
        let addr = mock_stalling_server().await;
        let (ws_stream, _respone) = tokio_tungstenite::connect_async(format!("ws://{addr}"))
            .await
            .unwrap();
        let (tx, mut rx) = mpsc::channel(8);
        tokio::spawn(async move { run(ws_stream, tx).await });

        let mut results = Vec::new();
        while let Some(result) = rx.recv().await {
            results.push(result);
        }

        // measurement + the timeout error
        assert!(results.len() >= 2);
        assert!(results[0].is_ok());
        assert!(matches!(
            results.last(),
            Some(Err(crate::error::Ndt7Error::Timeout(_)))
        ));
    }
}
