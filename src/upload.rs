//! ndt7 upload test implementation.
//!
//! Sends random binary WebSocket messages to the server while reading
//! server counter-flow measurements, until [`params::UPLOAD_TIMEOUT`] elapses.

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt, stream::SplitSink, stream::SplitStream};
use rand::RngCore;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use tokio::sync::mpsc;
use tokio::time::{Instant, timeout};
use tokio_tungstenite::tungstenite::Message;

use crate::client::WsStream;
use crate::error::{Ndt7Error, Result};
use crate::params;
use crate::spec::{AppInfo, Measurement, Origin, TestKind};

/// Run the upload test on an established WebSocket connection.
///
/// Measurements are sent on `tx` as they arrive. The function returns when
/// the timeout expires or the server closes the connection.
pub async fn run(ws: WsStream, tx: mpsc::Sender<Result<Measurement>>) {
    let (sink, stream) = ws.split();

    let result = tokio::select! {
       r = timeout(params::UPLOAD_TIMEOUT, upload_loop(sink, &tx)) => {
           match r {
               Ok(inner) => inner,
               Err(_) => Ok(()), // timeout is normal completion
           }
       }
       r = read_counterflow(stream, &tx) => r
    };

    if let Err(e) = result {
        let _ = tx.send(Err(e)).await;
    }
}

// Reads server counter-flow measurements
async fn read_counterflow(
    mut stream: SplitStream<WsStream>,
    tx: &mpsc::Sender<Result<Measurement>>,
) -> Result<()> {
    while let Some(msg) = stream.next().await {
        let msg = msg?;
        match msg {
            Message::Text(text) => {
                let mut measurement: Measurement = serde_json::from_str(&text)?;
                measurement.origin = Some(Origin::Server);
                measurement.test = Some(TestKind::Upload);
                let _ = tx.send(Ok(measurement)).await;
            }
            Message::Binary(_) => {
                return Err(Ndt7Error::ProtocolViolation(
                    "server sent unexpected binary message during upload".into(),
                ));
            }
            Message::Close(_) => break,
            _ => {} // Ping/Pong handled by tokio-tungstenite
        }
    }
    Ok(())
}

async fn upload_loop(
    mut sink: SplitSink<WsStream, Message>,
    tx: &mpsc::Sender<Result<Measurement>>,
) -> Result<()> {
    let start = Instant::now();
    let mut prev_update = start;
    let mut total_bytes: i64 = 0;

    let mut rng = SmallRng::from_os_rng();
    let mut msg_size = params::INITIAL_MESSAGE_SIZE;
    let mut buf = vec![0u8; msg_size];
    rng.fill_bytes(&mut buf);
    let mut payload = Bytes::from(buf);

    loop {
        sink.send(Message::Binary(payload.clone())).await?;
        total_bytes += payload.len() as i64;
        if msg_size < params::MAX_MESSAGE_SIZE
            && msg_size <= total_bytes as usize / params::SCALING_FRACTION
        {
            msg_size *= 2;
            let mut new_buf = vec![0u8; msg_size];
            rng.fill_bytes(&mut new_buf);
            payload = Bytes::from(new_buf);
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
                    test: Some(TestKind::Upload),
                    ..Default::default()
                }))
                .await;
        }
    }
}
