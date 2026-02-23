use serde::Serialize;

use crate::spec::{Measurement, TestKind};

#[derive(Debug, Clone, Serialize)]
pub struct SubtestSummary {
    pub throughput_mbps: f64,
    pub latency_ms: f64,
    pub retransmission_pct: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub server_fqdn: String,
    pub client_ip: String,
    pub server_ip: String,
    pub download: Option<SubtestSummary>,
    pub upload: Option<SubtestSummary>,
}

impl SubtestSummary {
    pub fn from_measurement(m: &Measurement, test: TestKind) -> Option<SubtestSummary> {
        let tcp = m.tcp_info.as_ref()?;
        let elapsed = tcp.elapsed_time? as f64;
        if elapsed <= 0.0 {
            return None;
        }

        let throughput_bytes = match test {
            TestKind::Download => tcp.bytes_acked.unwrap_or(0),
            TestKind::Upload => tcp.bytes_received.unwrap_or(0),
        } as f64;
        let throughput_mbps = 8.0 * throughput_bytes / elapsed;

        let latency_ms = tcp.min_rtt.unwrap_or(0) as f64 / 1000.0;

        let bytes_sent = tcp.bytes_sent.unwrap_or(0) as f64;
        let bytes_retrans = tcp.bytes_retrans.unwrap_or(0) as f64;
        let retransmission_pct = if bytes_sent > 0.0 {
            bytes_retrans / bytes_sent * 100.0
        } else {
            0.0
        };

        Some(SubtestSummary {
            throughput_mbps,
            latency_ms,
            retransmission_pct,
        })
    }
}

impl Summary {
    pub fn from_measurements(
        server_fqdn: String,
        download: Option<&Measurement>,
        upload: Option<&Measurement>,
    ) -> Summary {
        let conn = download.or(upload).and_then(|m| m.connection_info.as_ref());

        let client_ip = conn.map(|c| strip_port(&c.client)).unwrap_or_default();
        let server_ip = conn.map(|c| strip_port(&c.server)).unwrap_or_default();

        Summary {
            server_fqdn,
            client_ip,
            server_ip,
            download: download
                .and_then(|m| SubtestSummary::from_measurement(m, TestKind::Download)),
            upload: upload.and_then(|m| SubtestSummary::from_measurement(m, TestKind::Upload)),
        }
    }
}

fn strip_port(addr: &str) -> String {
    addr.parse::<std::net::SocketAddr>()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| addr.to_string())
}
