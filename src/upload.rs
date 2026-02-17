use futures_util::{SinkExt, StreamExt, stream::SplitSink, stream::SplitStream};
use rand::RngCore;
use rand::SeedableRng;
use rand::rngs::StdRng;
use tokio::sync::mpsc;
use tokio::time::{Instant, timeout};
use tokio_tungstenite::tungstenite::Message;

use crate::client::WsStream;
use crate::error::Result;
use crate::params;
use crate::spec::{AppInfo, Measurement, Origin, TestKind};

pub async fn run(ws: WsStream, tx: mpsc::Sender<Measurement>) -> Result<()> {
    let (sink, stream) = ws.split();

    tokio::select! {
       _ = timeout(params::UPLOAD_TIMEOUT, upload_loop(sink, &tx)) => {}
       _ = read_counterflow(stream, &tx) => {}
    }

    Ok(())
}

// Reads server counter-flow measurements
async fn read_counterflow(
    mut stream: SplitStream<WsStream>,
    tx: &mpsc::Sender<Measurement>,
) -> Result<()> {
    while let Some(msg) = stream.next().await {
        let msg = msg?;
        match msg {
            Message::Text(text) => {
                let mut measurement: Measurement = serde_json::from_str(&text)?;
                measurement.origin = Some(Origin::Server);
                measurement.test = Some(TestKind::Upload);
                let _ = tx.send(measurement).await;
            }
            Message::Close(_) => break,
            _ => {} // Ping/Pong handled by tokio-tungstenite
        }
    }
    Ok(())
}

async fn upload_loop(
    mut sink: SplitSink<WsStream, Message>,
    tx: &mpsc::Sender<Measurement>,
) -> Result<()> {
    let start = Instant::now();
    let mut prev_update = start;
    let mut total_bytes: i64 = 0;

    let mut rng = StdRng::from_os_rng();
    let mut msg_size = params::INITIAL_MESSAGE_SIZE;
    let mut buf = vec![0u8; msg_size];
    rng.fill_bytes(&mut buf);

    loop {
        sink.send(Message::Binary(buf.clone().into())).await?;
        total_bytes += buf.len() as i64;
        if msg_size < params::MAX_MESSAGE_SIZE
            && msg_size <= total_bytes as usize / params::SCALING_FRACTION
        {
            msg_size *= 2;
            buf = vec![0u8; msg_size];
            rng.fill_bytes(&mut buf);
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
                    test: Some(TestKind::Upload),
                    ..Default::default()
                })
                .await;
        }
    }
}
