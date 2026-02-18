use serde::Serialize;

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
