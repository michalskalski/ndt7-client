use clap::Parser;
use ndt7_client::client::Client;
use ndt7_client::emitter::{Emitter, HumanReadableEmitter, JsonEmitter};
use ndt7_client::error::Ndt7Error;
use ndt7_client::params;
use ndt7_client::spec::{Measurement, Origin, TestKind};
use ndt7_client::summary::Summary;

#[derive(Clone, Debug, clap::ValueEnum)]
enum Format {
    Human,
    Json,
}

#[derive(Parser, Debug)]
struct Cli {
    /// Optional ndt7 server hostname (e.g. localhost:8080). Bypasses locate API.
    #[arg(long)]
    server: Option<String>,
    /// Service URL specifies target hostname and other URL fields like access token. Overrides --server.
    #[arg(long)]
    service_url: Option<String>,
    /// Use unencrypted WebSocket (ws://) instead of TLS (wss://)
    #[arg(long)]
    no_tls: bool,
    /// Output format to use: 'human' or 'json' for batch processing
    #[arg(long, default_value = "human")]
    format: Format,
    /// Skip download measurement
    #[arg(long)]
    no_download: bool,
    /// Skip upload measurement
    #[arg(long)]
    no_upload: bool,
    /// Emit summary and errors only
    #[arg(long)]
    quiet: bool,
}

struct Targets {
    server_fqdn: String,
    download_url: Option<String>,
    upload_url: Option<String>,
}

async fn resolve_targets(
    cli: &Cli,
    client: &Client,
) -> Result<Targets, Box<dyn std::error::Error>> {
    let scheme = if cli.no_tls { "ws" } else { "wss" };

    if let Some(ref s) = cli.service_url {
        let parsed = url::Url::parse(s)?;
        let fqdn = parsed.host_str().ok_or(Ndt7Error::NoTargets)?.to_string();
        match parsed.path() {
            p if p.contains(params::DOWNLOAD_URL_PATH) => Ok(Targets {
                server_fqdn: fqdn,
                download_url: Some(s.clone()),
                upload_url: None,
            }),
            p if p.contains(params::UPLOAD_URL_PATH) => Ok(Targets {
                server_fqdn: fqdn,
                download_url: None,
                upload_url: Some(s.clone()),
            }),
            _ => Err(Ndt7Error::ServiceUnsupported(format!(
                "path must contain {} or {}",
                params::DOWNLOAD_URL_PATH,
                params::UPLOAD_URL_PATH
            ))
            .into()),
        }
    } else if let Some(ref server) = cli.server {
        Ok(Targets {
            server_fqdn: server.clone(),
            download_url: Some(format!("{scheme}://{server}{}", params::DOWNLOAD_URL_PATH))
                .filter(|_| !cli.no_download),
            upload_url: Some(format!("{scheme}://{server}{}", params::UPLOAD_URL_PATH))
                .filter(|_| !cli.no_upload),
        })
    } else {
        let locate = client.locate_test_targets().await?;
        Ok(Targets {
            server_fqdn: locate.server_fqdn,
            download_url: locate.download_url.filter(|_| !cli.no_download),
            upload_url: locate.upload_url.filter(|_| !cli.no_upload),
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let mut emitter: Box<dyn Emitter> = match cli.format {
        Format::Human => Box::new(HumanReadableEmitter::new(std::io::stdout())),
        Format::Json => Box::new(JsonEmitter::new(std::io::stdout())),
    };

    let client = Client::new("ndt7-client-rs".into(), env!("CARGO_PKG_VERSION").into());
    let targets = resolve_targets(&cli, &client).await?;

    if targets.download_url.is_none() && targets.upload_url.is_none() {
        eprintln!("error: nothing to do");
        std::process::exit(1);
    }

    let mut dl_client_measurement: Option<Measurement> = None;
    let mut dl_server_measurement: Option<Measurement> = None;
    let mut ul_measurement: Option<Measurement> = None;

    if let Some(ref url) = targets.download_url {
        let t = TestKind::Download;
        emitter.on_starting(t)?;
        let mut rx = client.start_download(url).await?;
        emitter.on_connected(t, &targets.server_fqdn)?;
        while let Some(m) = rx.recv().await {
            if !cli.quiet {
                emitter.on_download_event(&m)?;
            }
            match m.origin {
                Some(Origin::Client) => dl_client_measurement = Some(m),
                Some(Origin::Server) => dl_server_measurement = Some(m),
                None => {}
            }
        }
        emitter.on_complete(t)?;
    }

    if let Some(ref url) = targets.upload_url {
        let t = TestKind::Upload;
        emitter.on_starting(t)?;
        let mut rx = client.start_upload(url).await?;
        emitter.on_connected(t, &targets.server_fqdn)?;
        while let Some(m) = rx.recv().await {
            if !cli.quiet {
                emitter.on_upload_event(&m)?;
            }
            if m.origin == Some(Origin::Server) {
                ul_measurement = Some(m);
            }
        }
        emitter.on_complete(t)?;
    }

    let summary = Summary::from_measurements(
        targets.server_fqdn,
        dl_client_measurement.as_ref(),
        dl_server_measurement.as_ref(),
        ul_measurement.as_ref(),
    );

    emitter.on_summary(&summary)?;

    Ok(())
}
