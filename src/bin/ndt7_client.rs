use clap::Parser;
use ndt7_client::client::Client;
use ndt7_client::emitter::{Emitter, HumanReadableEmitter, JsonEmitter};
use ndt7_client::spec::{Measurement, Origin, TestKind};
use ndt7_client::summary::Summary;

#[derive(Clone, Debug, clap::ValueEnum)]
enum Format {
    Human,
    Json,
}

#[derive(Parser, Debug)]
struct Cli {
    /// Optional ndt7 server hostname
    #[arg(long)]
    server: Option<String>,
    /// Service URL specifies target hostname and other URL fields like access token. Overrides --server.
    #[arg(long)]
    service_url: Option<String>,
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

    if cli.no_download && cli.no_upload {
        eprintln!("error: nothing to do, both download and upload are disabled");
        std::process::exit(1);
    }

    let mut emitter: Box<dyn Emitter> = match cli.format {
        Format::Human => Box::new(HumanReadableEmitter::new(std::io::stdout())),
        Format::Json => Box::new(JsonEmitter::new(std::io::stdout())),
    };

    let client = Client::new("ndt7-client".into(), env!("CARGO_PKG_VERSION").into());

    let mut dl_measurement: Option<Measurement> = None;
    let mut ul_measurement: Option<Measurement> = None;
    let mut server_fqdn: Option<String> = None;

    if !cli.no_download {
        let t = TestKind::Download;
        emitter.on_starting(t)?;
        let (fqdn, mut rx) = client.start_download().await?;
        emitter.on_connected(t, &fqdn)?;
        server_fqdn = Some(fqdn);
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

    if !cli.no_upload {
        let t = TestKind::Upload;
        emitter.on_starting(t)?;
        let (fqdn, mut rx) = client.start_upload().await?;
        emitter.on_connected(t, &fqdn)?;
        server_fqdn = Some(fqdn);
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
        server_fqdn.unwrap_or_default(),
        dl_measurement.as_ref(),
        ul_measurement.as_ref(),
    );

    emitter.on_summary(&summary)?;

    Ok(())
}
