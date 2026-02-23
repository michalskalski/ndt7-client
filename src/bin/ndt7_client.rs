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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let mut emitter: Box<dyn Emitter> = match cli.format {
        Format::Human => Box::new(HumanReadableEmitter::new(std::io::stdout())),
        Format::Json => Box::new(JsonEmitter::new(std::io::stdout())),
    };

    let client = Client::new("ndt7-client-rs".into(), env!("CARGO_PKG_VERSION").into());

    let scheme = if cli.no_tls { "ws" } else { "wss" };

    let (server_fqdn, dl_url, ul_url) = if let Some(ref s) = cli.service_url {
        // user passed directly service url dictating what test to run
        let parsed = url::Url::parse(s)?;
        let fqdn = parsed.host_str().ok_or(Ndt7Error::NoTargets)?.to_string();
        match parsed.path() {
            p if p.contains(params::DOWNLOAD_URL_PATH) => (fqdn, Some(s.clone()), None),
            p if p.contains(params::UPLOAD_URL_PATH) => (fqdn, None, Some(s.clone())),
            _ => {
                eprintln!(
                    "error: service URL must contain {} or {}",
                    params::DOWNLOAD_URL_PATH,
                    params::UPLOAD_URL_PATH
                );
                std::process::exit(1);
            }
        }
    } else if let Some(ref server) = cli.server {
        // construct URLs from server hostname, bypass locate API
        let dl = if cli.no_download {
            None
        } else {
            Some(format!("{scheme}://{server}{}", params::DOWNLOAD_URL_PATH))
        };
        let ul = if cli.no_upload {
            None
        } else {
            Some(format!("{scheme}://{server}{}", params::UPLOAD_URL_PATH))
        };
        (server.clone(), dl, ul)
    } else {
        // discover service urls, filter based on user choices
        let locate = client.locate_test_targets().await?;
        let dl = if cli.no_download {
            None
        } else {
            locate.download_url
        };
        let ul = if cli.no_upload {
            None
        } else {
            locate.upload_url
        };
        (locate.server_fqdn, dl, ul)
    };

    if dl_url.is_none() && ul_url.is_none() {
        eprintln!("error: nothing to do");
        std::process::exit(1);
    }

    let mut dl_measurement: Option<Measurement> = None;
    let mut ul_measurement: Option<Measurement> = None;

    if let Some(ref url) = dl_url {
        let t = TestKind::Download;
        emitter.on_starting(t)?;
        let mut rx = client.start_download(url).await?;
        emitter.on_connected(t, &server_fqdn)?;
        while let Some(m) = rx.recv().await {
            if !cli.quiet {
                emitter.on_download_event(&m)?;
            }
            if m.origin == Some(Origin::Server) {
                dl_measurement = Some(m);
            }
        }
        emitter.on_complete(t)?;
    }

    if let Some(ref url) = ul_url {
        let t = TestKind::Upload;
        emitter.on_starting(t)?;
        let mut rx = client.start_upload(url).await?;
        emitter.on_connected(t, &server_fqdn)?;
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
        server_fqdn,
        dl_measurement.as_ref(),
        ul_measurement.as_ref(),
    );

    emitter.on_summary(&summary)?;

    Ok(())
}
