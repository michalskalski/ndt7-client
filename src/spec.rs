//! ndt7 protocol data types.
//!
//! These structs match the JSON measurement messages defined in the
//! [ndt7 specification](https://github.com/m-lab/ndt-server/blob/master/spec/ndt7-protocol.md).

use serde::{Deserialize, Serialize};

/// Which side produced a measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    /// Measurement computed by the client.
    Client,
    /// Measurement reported by the server.
    Server,
}

/// Which subtest a measurement belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestKind {
    /// Download (server-to-client) test.
    Download,
    /// Upload (client-to-server) test.
    Upload,
}

/// Application-level throughput counters.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppInfo {
    /// Microseconds elapsed since the start of the test.
    #[serde(rename = "ElapsedTime")]
    pub elapsed_time: i64,
    /// Total bytes transferred so far.
    #[serde(rename = "NumBytes")]
    pub num_bytes: i64,
}

/// Endpoint addresses and connection metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionInfo {
    /// Client address as `ip:port`.
    #[serde(rename = "Client")]
    pub client: String,
    /// Server address as `ip:port`.
    #[serde(rename = "Server")]
    pub server: String,
    /// Unique identifier for the test assigned by the server.
    #[serde(rename = "UUID", skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// Start time of the test in RFC 3339 format.
    #[serde(rename = "StartTime", skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
}

/// TCP connection metrics from the kernel (server-side).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TCPInfo {
    /// Time (microseconds) the connection has been actively sending data.
    #[serde(rename = "BusyTime", skip_serializing_if = "Option::is_none")]
    pub busy_time: Option<i64>,
    /// Bytes acknowledged by the peer.
    #[serde(rename = "BytesAcked", skip_serializing_if = "Option::is_none")]
    pub bytes_acked: Option<i64>,
    /// Bytes received from the peer.
    #[serde(rename = "BytesReceived", skip_serializing_if = "Option::is_none")]
    pub bytes_received: Option<i64>,
    /// Bytes sent to the peer.
    #[serde(rename = "BytesSent", skip_serializing_if = "Option::is_none")]
    pub bytes_sent: Option<i64>,
    /// Bytes retransmitted.
    #[serde(rename = "BytesRetrans", skip_serializing_if = "Option::is_none")]
    pub bytes_retrans: Option<i64>,
    /// Microseconds elapsed since the TCP connection was established.
    #[serde(rename = "ElapsedTime", skip_serializing_if = "Option::is_none")]
    pub elapsed_time: Option<i64>,
    /// Minimum round-trip time observed (microseconds).
    #[serde(rename = "MinRTT", skip_serializing_if = "Option::is_none")]
    pub min_rtt: Option<i64>,
    /// Smoothed round-trip time (microseconds).
    #[serde(rename = "RTT", skip_serializing_if = "Option::is_none")]
    pub rtt: Option<i64>,
    /// Round-trip time variance (microseconds).
    #[serde(rename = "RTTVar", skip_serializing_if = "Option::is_none")]
    pub rtt_var: Option<i64>,
    /// Time (microseconds) limited by the receive window.
    #[serde(rename = "RWndLimited", skip_serializing_if = "Option::is_none")]
    pub rwnd_limited: Option<i64>,
    /// Time (microseconds) limited by the send buffer.
    #[serde(rename = "SndBufLimited", skip_serializing_if = "Option::is_none")]
    pub snd_buf_limited: Option<i64>,
}

/// A single measurement message exchanged during an ndt7 test.
///
/// Both the server and client produce measurements. Server measurements
/// include [`TCPInfo`] from the kernel; client measurements include
/// [`AppInfo`] with application-level byte counts.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    /// Application-level throughput counters.
    #[serde(rename = "AppInfo", skip_serializing_if = "Option::is_none")]
    pub app_info: Option<AppInfo>,
    /// Connection endpoint addresses.
    #[serde(rename = "ConnectionInfo", skip_serializing_if = "Option::is_none")]
    pub connection_info: Option<ConnectionInfo>,
    /// Which side produced this measurement.
    #[serde(rename = "Origin", skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
    /// Which subtest this measurement belongs to.
    #[serde(rename = "Test", skip_serializing_if = "Option::is_none")]
    pub test: Option<TestKind>,
    /// TCP-level metrics from the kernel.
    #[serde(rename = "TCPInfo", skip_serializing_if = "Option::is_none")]
    pub tcp_info: Option<TCPInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_measurement() {
        let measurement = Measurement {
            app_info: Some(AppInfo::default()),
            origin: Some(Origin::Client),
            ..Default::default()
        };

        let json_output = serde_json::to_string(&measurement).unwrap();
        assert!(json_output.contains(r#""AppInfo""#));
        assert!(json_output.contains(r#""ElapsedTime""#));
        assert!(json_output.contains(r#""Origin":"client""#));

        // Omited fields are absent
        assert!(!json_output.contains("TCPInfo"));
        assert!(!json_output.contains("ConnectionInfo"));
    }

    #[test]
    fn non_fields_omitted() {
        let json_output = serde_json::to_string(&Measurement::default()).unwrap();
        assert_eq!(json_output, "{}");
    }

    #[test]
    fn deserialize_protocol_spec() {
        let json = r#"{
            "AppInfo": {"ElapsedTime": 1234, "NumBytes": 5678},
            "ConnectionInfo": {"Client": "1.2.3.4:5678", "Server": "[::1]:2345", "UUID": "abc-1234"},
            "Origin": "server",
            "Test": "download",
            "TCPInfo": {"RTT": 6000, "MinRTT": 5000}
        }"#;
        let m: Measurement = serde_json::from_str(json).unwrap();

        let app = m.app_info.unwrap();
        assert_eq!(app.elapsed_time, 1234);
        assert_eq!(app.num_bytes, 5678);

        let con_info = m.connection_info.unwrap();
        let uuid = con_info.uuid.unwrap();
        assert_eq!(con_info.client, "1.2.3.4:5678");
        assert_eq!(con_info.server, "[::1]:2345");
        assert_eq!(uuid, "abc-1234");

        let origin = m.origin.unwrap();
        assert_eq!(origin, Origin::Server);

        let test = m.test.unwrap();
        assert_eq!(test, TestKind::Download);

        let tcp_info = m.tcp_info.unwrap();
        let rtt = tcp_info.rtt.unwrap();
        let min_rtt = tcp_info.min_rtt.unwrap();
        assert_eq!(rtt, 6000);
        assert_eq!(min_rtt, 5000);
    }

    #[test]
    fn round_trip() {
        let m = Measurement {
            app_info: Some(AppInfo {
                elapsed_time: 500_000,
                num_bytes: 1_048_576,
            }),
            connection_info: Some(ConnectionInfo {
                client: "10.0.0.1:12345".into(),
                server: "10.0.0.2:443".into(),
                uuid: Some("test-uuid".into()),
                start_time: Some("2026-02-23T13:05:00.000000000Z".into()),
            }),
            origin: Some(Origin::Server),
            test: Some(TestKind::Upload),
            tcp_info: Some(TCPInfo {
                rtt: Some(10_000),
                min_rtt: Some(8_000),
                ..Default::default()
            }),
        };

        let json = serde_json::to_string(&m).unwrap();
        let deserialized: Measurement = serde_json::from_str(&json).unwrap();
        assert_eq!(m, deserialized);
    }
}
