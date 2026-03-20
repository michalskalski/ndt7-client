//! High-level ndt7 test client.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::Request;
use tokio_tungstenite::{Connector, MaybeTlsStream, client_async_tls_with_config};
use url::Url;

use crate::download;
use crate::error::{Ndt7Error, Result};
use crate::locate::Target;
use crate::spec::{Measurement, TestKind};
use crate::upload;
use crate::{locate, params};

/// A certificate verifier that accepts any certificate.
/// Used with --no-verify for testing against servers with self-signed certs.
#[derive(Debug)]
struct NoVerifier;

impl rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::aws_lc_rs::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Type alias for the WebSocket stream used by download and upload tests.
pub type WsStream = tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>;

/// IP address family preference for test connections.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum AddressFamily {
    /// Use whichever address family DNS resolution returns first.
    #[default]
    Any,
    /// Force IPv4 connections only.
    Ipv4Only,
    /// Force IPv6 connections only.
    Ipv6Only,
}

impl AddressFamily {
    /// Pick the first address matching this family, or `None` if no match.
    pub fn select_addr(&self, addrs: impl Iterator<Item = SocketAddr>) -> Option<SocketAddr> {
        match self {
            AddressFamily::Any => addrs.into_iter().next(),
            AddressFamily::Ipv4Only => addrs.into_iter().find(|a| a.is_ipv4()),
            AddressFamily::Ipv6Only => addrs.into_iter().find(|a| a.is_ipv6()),
        }
    }
}

impl std::fmt::Display for AddressFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddressFamily::Any => write!(f, "any"),
            AddressFamily::Ipv4Only => write!(f, "IPv4"),
            AddressFamily::Ipv6Only => write!(f, "IPv6"),
        }
    }
}

/// Handle to a running ndt7 test, returned by [`Client::start_download`] and [`Client::start_upload`].
pub struct TestHandle {
    /// Fully qualified domain name of the server running the test.
    pub server_fqdn: String,
    /// Channel of measurement results from the running test.
    pub rx: mpsc::Receiver<Result<Measurement>>,
}

/// An ndt7 test client.
///
/// Use [`ClientBuilder`] to create a client, then [`Client::start_download`] /
/// [`Client::start_upload`] to run tests. Pass `None` to auto-locate the nearest
/// M-Lab server with retry, or `Some(url)` for a specific server.
pub struct Client {
    client_name: String,
    client_version: String,
    no_verify_tls: bool,
    no_tls: bool,
    address_family: AddressFamily,
    targets: Option<Vec<Target>>,
}

/// Builder for [`Client`].
///
/// ```
/// # use ndt7_client::client::ClientBuilder;
/// let client = ClientBuilder::new("my-app", "1.0.0").build();
/// ```
pub struct ClientBuilder {
    client_name: String,
    client_version: String,
    no_verify_tls: bool,
    no_tls: bool,
    address_family: AddressFamily,
}

impl ClientBuilder {
    /// Create a new builder. `client_name` and `client_version` identify the
    /// calling application in requests to M-Lab servers.
    pub fn new(client_name: impl Into<String>, client_version: impl Into<String>) -> Self {
        ClientBuilder {
            client_name: client_name.into(),
            client_version: client_version.into(),
            no_verify_tls: false,
            no_tls: false,
            address_family: AddressFamily::Any,
        }
    }

    /// Skip TLS certificate verification.
    pub fn no_verify_tls(mut self) -> Self {
        self.no_verify_tls = true;
        self
    }

    /// Use unencrypted ws:// connection
    pub fn no_tls(mut self) -> Self {
        self.no_tls = true;
        self
    }

    /// Set the preferred IP address family for connections.
    pub fn address_family(mut self, af: AddressFamily) -> Self {
        self.address_family = af;
        self
    }

    /// Build the [`Client`].
    pub fn build(self) -> Client {
        Client {
            client_name: self.client_name,
            client_version: self.client_version,
            no_verify_tls: self.no_verify_tls,
            no_tls: self.no_tls,
            address_family: self.address_family,
            targets: None,
        }
    }
}

impl Client {
    /// Establish a WebSocket connection to the given service URL.
    ///
    /// `service_url` is the full URL from the Locate API, e.g.
    /// "wss://mlab1-lga06:4443/ndt/v7/download?access_token=..."
    pub async fn connect(&self, service_url: &str) -> Result<WsStream> {
        // Parse the URL and append client metadata as query parameters.
        let mut url = Url::parse(service_url)?;
        url.query_pairs_mut()
            .append_pair("client_name", &self.client_name)
            .append_pair("client_version", &self.client_version)
            .append_pair("client_os", std::env::consts::OS)
            .append_pair("client_arch", std::env::consts::ARCH)
            .append_pair(
                "client_library_name",
                &format!("{}-rs", env!("CARGO_PKG_NAME")),
            )
            .append_pair("client_library_version", env!("CARGO_PKG_VERSION"));

        // Build the HTTP request with required headers.
        let mut request = url.to_string().into_client_request()?;
        request.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            params::SEC_WEBSOCKET_PROTOCOL.parse().unwrap(),
        );
        request
            .headers_mut()
            .insert("User-Agent", self.user_agent().parse().unwrap());

        timeout(params::IO_TIMEOUT, self.connect_ws(request, &url)).await?
    }

    async fn connect_ws(&self, request: Request<()>, url: &Url) -> Result<WsStream> {
        let connector = (url.scheme() == "wss").then(|| self.tls_connector());

        // DNS resolution
        let host = url
            .host_str()
            .ok_or(Ndt7Error::ServiceUnsupported("missing host in URL".into()))?;
        let port = url
            .port_or_known_default()
            .ok_or(Ndt7Error::ServiceUnsupported("missing port".into()))?;
        let addrs = tokio::net::lookup_host((host, port)).await?;

        // Filter by address family
        let addr = self
            .address_family
            .select_addr(addrs)
            .ok_or(Ndt7Error::NoAddressFound(self.address_family))?;

        // TCP + TLS + WebSocket
        let tcp = TcpStream::connect(addr).await?;
        let (ws_stream, _response) =
            client_async_tls_with_config(request, tcp, None, connector).await?;

        Ok(ws_stream)
    }

    /// Start a download test and return a channel of [`Measurement`] results.
    ///
    /// The test runs in a background task. Each item is `Ok(measurement)` or
    /// `Err(error)` if the test fails mid-stream. An error is always the last
    /// item - the channel closes immediately after.
    pub async fn start_download(&mut self, url: Option<&str>) -> Result<TestHandle> {
        let (ws, server_fqdn) = self.connect_with_retry(url, TestKind::Download).await?;
        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(async move {
            download::run(ws, tx).await;
        });
        Ok(TestHandle { server_fqdn, rx })
    }

    /// Start an upload test and return a channel of [`Measurement`] results.
    ///
    /// The test runs in a background task. Each item is `Ok(measurement)` or
    /// `Err(error)` if the test fails mid-stream. An error is always the last
    /// item - the channel closes immediately after.
    pub async fn start_upload(&mut self, url: Option<&str>) -> Result<TestHandle> {
        let (ws, server_fqdn) = self.connect_with_retry(url, TestKind::Upload).await?;
        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(async move {
            upload::run(ws, tx).await;
        });
        Ok(TestHandle { server_fqdn, rx })
    }

    async fn connect_with_retry(
        &mut self,
        url: Option<&str>,
        test_kind: TestKind,
    ) -> Result<(WsStream, String)> {
        if let Some(url) = url {
            let ws = self.connect(url).await?;
            let fqdn = Url::parse(url)?.host_str().unwrap_or("unknown").to_string();
            Ok((ws, fqdn))
        } else {
            let scheme = if self.no_tls { "ws" } else { "wss" };
            let mut last_err = Ndt7Error::NoTargets;
            let targets = self.get_targets().await?.to_vec();
            for t in &targets {
                let url = match test_kind {
                    TestKind::Download => t.service_urls(scheme).download,
                    TestKind::Upload => t.service_urls(scheme).upload,
                };
                let Some(url) = url else { continue };
                match self.connect(&url).await {
                    Ok(ws) => return Ok((ws, t.machine.clone())),
                    Err(e) => {
                        last_err = e;
                    }
                }
            }
            Err(last_err)
        }
    }

    async fn get_targets(&mut self) -> Result<&[Target]> {
        if self.targets.is_none() {
            self.targets = Some(locate::nearest(&self.user_agent()).await?);
        }
        Ok(self.targets.as_deref().unwrap())
    }

    fn tls_connector(&self) -> Connector {
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let tls_config = if self.no_verify_tls {
            rustls::ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .unwrap()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(NoVerifier))
                .with_no_client_auth()
        } else {
            let root_store =
                rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            rustls::ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_root_certificates(root_store)
                .with_no_client_auth()
        };
        Connector::Rustls(Arc::new(tls_config))
    }

    fn user_agent(&self) -> String {
        format!(
            "{}/{} {}-rs/{}",
            &self.client_name,
            &self.client_version,
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        )
    }

    #[cfg(test)]
    fn set_targets(&mut self, targets: Vec<Target>) {
        self.targets = Some(targets);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use futures_util::{SinkExt, StreamExt};
    use std::{collections::HashMap, net::SocketAddr};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};

    fn addr(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn select_addr_any_returns_first() {
        let addrs = vec![addr("127.0.0.1:443"), addr("[::1]:443")];
        let result = AddressFamily::Any.select_addr(addrs.into_iter());
        assert_eq!(result, Some(addr("127.0.0.1:443")));
    }

    #[test]
    fn select_addr_ipv4_skips_ipv6() {
        let addrs = vec![addr("[::1]:443"), addr("127.0.0.1:443")];
        let result = AddressFamily::Ipv4Only.select_addr(addrs.into_iter());
        assert_eq!(result, Some(addr("127.0.0.1:443")));
    }

    #[test]
    fn select_addr_ipv6_skips_ipv4() {
        let addrs = vec![addr("127.0.0.1:443"), addr("[::1]:443")];
        let result = AddressFamily::Ipv6Only.select_addr(addrs.into_iter());
        assert_eq!(result, Some(addr("[::1]:443")));
    }

    #[test]
    fn select_addr_ipv4_none_when_only_ipv6() {
        let addrs = vec![addr("[::1]:443"), addr("[::2]:443")];
        let result = AddressFamily::Ipv4Only.select_addr(addrs.into_iter());
        assert_eq!(result, None);
    }

    #[test]
    fn select_addr_ipv6_none_when_only_ipv4() {
        let addrs = vec![addr("127.0.0.1:443"), addr("10.0.0.1:443")];
        let result = AddressFamily::Ipv6Only.select_addr(addrs.into_iter());
        assert_eq!(result, None);
    }

    #[test]
    fn select_addr_empty_returns_none() {
        let addrs: Vec<SocketAddr> = vec![];
        assert_eq!(
            AddressFamily::Any.select_addr(addrs.clone().into_iter()),
            None
        );
        assert_eq!(
            AddressFamily::Ipv4Only.select_addr(addrs.clone().into_iter()),
            None
        );
        assert_eq!(AddressFamily::Ipv6Only.select_addr(addrs.into_iter()), None);
    }

    async fn mock_refusing_server() -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            drop(stream);
        });
        addr
    }

    async fn mock_server() -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            #[allow(clippy::result_large_err)]
            let ws_stream =
                tokio_tungstenite::accept_hdr_async(stream, |req: &Request, mut resp: Response| {
                    // to mitigate SecWebSocketSubProtocolError(NoSubProtocol)
                    if let Some(proto) = req.headers().get("Sec-WebSocket-Protocol") {
                        resp.headers_mut()
                            .insert("Sec-WebSocket-Protocol", proto.clone());
                    }
                    Ok(resp)
                })
                .await
                .unwrap();
            let (mut sink, _stream) = ws_stream.split();
            sink.send(Message::Text(
                r#"{"AppInfo":{"ElapsedTime":1000,"NumBytes":8192}}"#.into(),
            ))
            .await
            .unwrap();

            sink.send(Message::Close(None)).await.unwrap();
        });
        addr
    }

    fn local_target(addr: &std::net::SocketAddr) -> Target {
        let machine = addr.ip().to_string();
        let urls = HashMap::from([(
            "ws:///ndt/v7/download".into(),
            format!("ws://{addr}/ndt/v7/download"),
        )]);
        Target {
            machine,
            urls,
            location: None,
        }
    }

    #[tokio::test]
    async fn test_retry() {
        let bad_server = mock_refusing_server().await;
        let good_server = mock_server().await;
        let targets = vec![local_target(&bad_server), local_target(&good_server)];

        let mut client = ClientBuilder::new("test", "test").no_tls().build();
        client.set_targets(targets);

        let mut results = Vec::new();
        let handle = client.start_download(None).await.unwrap();

        assert_eq!(handle.server_fqdn, good_server.ip().to_string());
        let mut rx = handle.rx;

        while let Some(result) = rx.recv().await {
            results.push(result);
        }

        assert!(!results.is_empty());
        assert!(results[0].is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_download_real_server() {
        let mut client = ClientBuilder::new("ndt7-client-rust", env!("CARGO_PKG_VERSION")).build();
        let handle = client.start_download(None).await.unwrap();
        println!("connected to {}", handle.server_fqdn);
        let mut rx = handle.rx;

        let mut count = 0;
        while let Some(m) = rx.recv().await {
            count += 1;
            println!("{:?}", m);
        }
        assert!(count > 0);
    }

    #[tokio::test]
    #[ignore]
    async fn test_upload_real_server() {
        let mut client = ClientBuilder::new("ndt7-client-rust", env!("CARGO_PKG_VERSION")).build();
        let handle = client.start_upload(None).await.unwrap();
        println!("connected to {}", handle.server_fqdn);
        let mut rx = handle.rx;

        let mut count = 0;
        while let Some(m) = rx.recv().await {
            count += 1;
            println!("{:?}", m);
        }
        assert!(count > 0);
    }
}
