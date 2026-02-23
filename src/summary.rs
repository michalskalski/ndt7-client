use serde::Serialize;

use crate::spec::Measurement;

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
    /// Build download summary: throughput from client AppInfo, latency/retransmission from server TCPInfo.
    pub fn from_download(client: &Measurement, server: &Measurement) -> Option<SubtestSummary> {
        let app = client.app_info.as_ref()?;
        if app.elapsed_time <= 0 {
            return None;
        }
        let throughput_mbps = 8.0 * app.num_bytes as f64 / app.elapsed_time as f64;

        let tcp = server.tcp_info.as_ref();
        let latency_ms = tcp.and_then(|t| t.min_rtt).unwrap_or(0) as f64 / 1000.0;

        let bytes_sent = tcp.and_then(|t| t.bytes_sent).unwrap_or(0) as f64;
        let bytes_retrans = tcp.and_then(|t| t.bytes_retrans).unwrap_or(0) as f64;
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

    /// Build upload summary: throughput/latency/retransmission all from server TCPInfo.
    pub fn from_upload(server: &Measurement) -> Option<SubtestSummary> {
        let tcp = server.tcp_info.as_ref()?;
        let elapsed = tcp.elapsed_time? as f64;
        if elapsed <= 0.0 {
            return None;
        }

        let throughput_mbps = 8.0 * tcp.bytes_received.unwrap_or(0) as f64 / elapsed;
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
        dl_client: Option<&Measurement>,
        dl_server: Option<&Measurement>,
        ul_server: Option<&Measurement>,
    ) -> Summary {
        let conn = dl_server
            .or(ul_server)
            .and_then(|m| m.connection_info.as_ref());

        let client_ip = conn.map(|c| strip_port(&c.client)).unwrap_or_default();
        let server_ip = conn.map(|c| strip_port(&c.server)).unwrap_or_default();

        let download = dl_client.zip(dl_server).and_then(|(c, s)| {
            SubtestSummary::from_download(c, s)
        });

        Summary {
            server_fqdn,
            client_ip,
            server_ip,
            download,
            upload: ul_server.and_then(SubtestSummary::from_upload),
        }
    }
}

fn strip_port(addr: &str) -> String {
    addr.parse::<std::net::SocketAddr>()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| addr.to_string())
}
