use std::sync::Arc;

use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{Connector, MaybeTlsStream, connect_async_tls_with_config};
use url::Url;

use crate::download;
use crate::error::{Ndt7Error, Result};
use crate::spec::Measurement;
use crate::upload;
use crate::{locate, params};

/// Type alias for the WebSocket stream
pub type WsStream = tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct Client {
    pub client_name: String,
    pub client_version: String,
}

impl Client {
    pub fn new(client_name: String, client_version: String) -> Self {
        Client {
            client_name,
            client_version,
        }
    }

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
            .append_pair("client_arch", std::env::consts::ARCH);

        // Build the HTTP request with the WebSocket subprotocol header.
        let mut request = url.to_string().into_client_request()?;
        request.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            params::SEC_WEBSOCKET_PROTOCOL.parse().unwrap(),
        );

        // Connect using rustls for TLS.
        let root_store =
            rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls_config = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(root_store)
        .with_no_client_auth();

        let connector = Connector::Rustls(Arc::new(tls_config));
        let (ws_stream, _response) =
            connect_async_tls_with_config(request, None, false, Some(connector)).await?;

        Ok(ws_stream)
    }

    pub async fn locate_test_targets(&self) -> Result<LocateResult> {
        let targets = locate::nearest(&self.user_agent()).await?;
        let target = targets.into_iter().next().ok_or(Ndt7Error::NoTargets)?;

        let mut dl_url: Option<String> = None;
        let mut ul_url: Option<String> = None;

        for (key, url) in target.urls {
            if key.contains(params::DOWNLOAD_URL_PATH) {
                dl_url = Some(url);
            } else if key.contains(params::UPLOAD_URL_PATH) {
                ul_url = Some(url);
            }
        }

        Ok(LocateResult {
            server_fqdn: target.machine,
            download_url: dl_url,
            upload_url: ul_url,
        })
    }

    pub async fn start_download(&self, url: &str) -> Result<mpsc::Receiver<Measurement>> {
        // connect
        let ws = self.connect(url).await?;

        // spawn download task, return receiver
        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(async move {
            let _ = download::run(ws, tx).await;
        });
        Ok(rx)
    }

    pub async fn start_upload(&self, url: &str) -> Result<mpsc::Receiver<Measurement>> {
        // connect
        let ws = self.connect(url).await?;

        // spawn upload task, return receiver
        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(async move {
            let _ = upload::run(ws, tx).await;
        });
        Ok(rx)
    }

    fn user_agent(&self) -> String {
        format!(
            "{}/{} {}/{}",
            &self.client_name,
            &self.client_version,
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        )
    }
}

pub struct LocateResult {
    pub server_fqdn: String,
    pub download_url: Option<String>,
    pub upload_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_download_real_server() {
        let client = Client::new("ndt7-client-rust".into(), "0.1.0".into());
        let locate = client.locate_test_targets().await.unwrap();
        let mut rx = client
            .start_download(&locate.download_url.unwrap())
            .await
            .unwrap();

        let mut count = 0;
        println!("connected to {}", locate.server_fqdn);
        while let Some(m) = rx.recv().await {
            count += 1;
            println!("{:?}", m);
        }
        assert!(count > 0);
    }

    #[tokio::test]
    #[ignore]
    async fn test_upload_real_server() {
        let client = Client::new("ndt7-client-rust".into(), "0.1.0".into());
        let locate = client.locate_test_targets().await.unwrap();
        let mut rx = client
            .start_upload(&locate.upload_url.unwrap())
            .await
            .unwrap();

        let mut count = 0;
        println!("connected to {}", locate.server_fqdn);
        while let Some(m) = rx.recv().await {
            count += 1;
            println!("{:?}", m);
        }
        assert!(count > 0);
    }
}
