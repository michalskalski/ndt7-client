use std::io;
use std::io::Write;
use std::process::exit;

use clap::Parser;
use ndt7_client::client::{AddressFamily, Client, ClientBuilder};
use ndt7_client::emitter::{Emitter, HumanReadableEmitter, JsonEmitter};
use ndt7_client::error::Ndt7Error;
use ndt7_client::locate::Target;
use ndt7_client::spec::{Measurement, Origin, TestKind};
use ndt7_client::summary::Summary;
use ndt7_client::{locate, params};

const CLIENT_NAME: &str = "ndt7-client-rs";

#[derive(Clone, Debug, clap::ValueEnum)]
enum Format {
    Human,
    Json,
}

#[derive(Parser, Debug)]
struct Cli {
    /// Server hostname. With --no-locate: connect directly (e.g. localhost:8080).
    /// Without --no-locate: select this server via locate API (gets access tokens).
    /// With no value: interactive server picker.
    #[arg(long, group = "server_selection", num_args = 0..=1, default_missing_value = "")]
    server: Option<String>,
    /// Full service URL with path and access token. For advanced use / scripting.
    #[arg(long, group = "server_selection")]
    service_url: Option<String>,
    /// Skip locate API, connect directly to the server specified by --server
    #[arg(long, requires = "server")]
    no_locate: bool,
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
    /// Skip tls certificate verification
    #[arg(long)]
    no_verify: bool,
    /// List available target servers and exit
    #[arg(long)]
    list_servers: bool,
    /// Force IPv4 connections
    #[arg(long, group = "ip_version")]
    ipv4: bool,
    /// Force IPv6 connections
    #[arg(long, group = "ip_version")]
    ipv6: bool,
}

struct Targets {
    server_fqdn: String,
    download_url: Option<String>,
    upload_url: Option<String>,
}

fn user_agent() -> String {
    format!("{CLIENT_NAME}/{}", env!("CARGO_PKG_VERSION"))
}

/// Parse a --service-url into download or upload target based on its path.
fn resolve_from_service_url(url: &str) -> Result<Targets, Box<dyn std::error::Error>> {
    let parsed = url::Url::parse(url)?;
    let fqdn = parsed.host_str().ok_or(Ndt7Error::NoTargets)?.to_string();
    match parsed.path() {
        p if p.contains(params::DOWNLOAD_URL_PATH) => Ok(Targets {
            server_fqdn: fqdn,
            download_url: Some(url.to_string()),
            upload_url: None,
        }),
        p if p.contains(params::UPLOAD_URL_PATH) => Ok(Targets {
            server_fqdn: fqdn,
            download_url: None,
            upload_url: Some(url.to_string()),
        }),
        _ => Err(Ndt7Error::ServiceUnsupported(format!(
            "path must contain {} or {}",
            params::DOWNLOAD_URL_PATH,
            params::UPLOAD_URL_PATH
        ))
        .into()),
    }
}

/// Build URLs for a direct connection (no locate API, no tokens).
fn resolve_direct(server: &str, scheme: &str, no_download: bool, no_upload: bool) -> Targets {
    Targets {
        server_fqdn: server.to_string(),
        download_url: Some(format!("{scheme}://{server}{}", params::DOWNLOAD_URL_PATH))
            .filter(|_| !no_download),
        upload_url: Some(format!("{scheme}://{server}{}", params::UPLOAD_URL_PATH))
            .filter(|_| !no_upload),
    }
}

/// Call locate API, present interactive picker, return chosen server's URLs.
async fn resolve_interactive(
    scheme: &str,
    no_download: bool,
    no_upload: bool,
) -> Result<Targets, Box<dyn std::error::Error>> {
    let targets = locate::nearest(&user_agent()).await?;
    print_targets(&targets);
    let target = loop {
        print!("Select server [1-{}]: ", targets.len());
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        match input.trim().parse::<usize>() {
            Ok(n) if n >= 1 && n <= targets.len() => break &targets[n - 1],
            Ok(_) => println!("choice out of range"),
            Err(_) => println!("enter a number"),
        }
    };
    let urls = target.service_urls(scheme);
    Ok(Targets {
        server_fqdn: target.machine.clone(),
        download_url: urls.download.filter(|_| !no_download),
        upload_url: urls.upload.filter(|_| !no_upload),
    })
}

/// Call locate API, find a specific server by hostname, return its URLs with tokens.
async fn resolve_from_locate(
    server: &str,
    scheme: &str,
    no_download: bool,
    no_upload: bool,
) -> Result<Targets, Box<dyn std::error::Error>> {
    let targets = locate::nearest(&user_agent()).await?;
    let target = targets
        .iter()
        .find(|t| t.machine == server)
        .ok_or_else(|| {
            Ndt7Error::ServiceUnsupported(format!(
                "server '{}' not found in locate results; use --list-servers to see available servers",
                server
            ))
        })?;
    let urls = target.service_urls(scheme);
    Ok(Targets {
        server_fqdn: target.machine.clone(),
        download_url: urls.download.filter(|_| !no_download),
        upload_url: urls.upload.filter(|_| !no_upload),
    })
}

/// Call locate API, pick nearest server automatically.
async fn resolve_nearest(
    client: &Client,
    no_download: bool,
    no_upload: bool,
) -> Result<Targets, Box<dyn std::error::Error>> {
    let locate = client.locate_test_targets().await?;
    Ok(Targets {
        server_fqdn: locate.server_fqdn,
        download_url: locate.download_url.filter(|_| !no_download),
        upload_url: locate.upload_url.filter(|_| !no_upload),
    })
}

async fn resolve_targets(
    cli: &Cli,
    client: &Client,
) -> Result<Targets, Box<dyn std::error::Error>> {
    let scheme = if cli.no_tls { "ws" } else { "wss" };

    if let Some(ref url) = cli.service_url {
        resolve_from_service_url(url)
    } else if let Some(ref server) = cli.server {
        if cli.no_locate {
            Ok(resolve_direct(
                server,
                scheme,
                cli.no_download,
                cli.no_upload,
            ))
        } else if server.is_empty() {
            resolve_interactive(scheme, cli.no_download, cli.no_upload).await
        } else {
            resolve_from_locate(server, scheme, cli.no_download, cli.no_upload).await
        }
    } else {
        resolve_nearest(client, cli.no_download, cli.no_upload).await
    }
}

fn print_targets(targets: &[Target]) {
    println!("{:<4} {:<65} Location", "#", "Server");
    for (pos, target) in targets.iter().enumerate() {
        let location = match &target.location {
            Some(loc) if !loc.city.is_empty() => format!("{}, {}", loc.city, loc.country),
            Some(loc) => loc.country.clone(),
            None => "-".to_string(),
        };
        println!("{:<4} {:<65} {}", pos + 1, target.machine, location);
    }
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("\nerror: {e}");
        exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if cli.no_locate && cli.server.as_deref() == Some("") {
        eprintln!("error: --no-locate requires a server hostname");
        exit(1);
    }

    if cli.list_servers {
        let targets = locate::nearest(&user_agent()).await?;
        if targets.is_empty() {
            eprintln!("no targets");
            exit(1)
        }
        match cli.format {
            Format::Human => print_targets(&targets),
            Format::Json => {
                let out = serde_json::to_string_pretty(&targets)?;
                println!("{out}")
            }
        }
        return Ok(());
    }

    let mut emitter: Box<dyn Emitter> = match cli.format {
        Format::Human => Box::new(HumanReadableEmitter::new(std::io::stdout())),
        Format::Json => Box::new(JsonEmitter::new(std::io::stdout())),
    };

    let mut builder = ClientBuilder::new(CLIENT_NAME, env!("CARGO_PKG_VERSION"));
    if cli.no_verify {
        builder = builder.no_verify_tls();
    }
    if cli.no_tls {
        builder = builder.no_tls();
    }
    let af = match (cli.ipv4, cli.ipv6) {
        (true, _) => AddressFamily::Ipv4Only,
        (_, true) => AddressFamily::Ipv6Only,
        _ => AddressFamily::Any,
    };
    let client = builder.address_family(af).build();
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
        while let Some(result) = rx.recv().await {
            match result {
                Ok(m) => {
                    if !cli.quiet {
                        emitter.on_download_event(&m)?;
                    }
                    match m.origin {
                        Some(Origin::Client) => dl_client_measurement = Some(m),
                        Some(Origin::Server) => dl_server_measurement = Some(m),
                        None => {}
                    }
                }
                Err(e) => emitter.on_error(t, &e.to_string())?,
            }
        }
        emitter.on_complete(t)?;
    }

    if let Some(ref url) = targets.upload_url {
        let t = TestKind::Upload;
        emitter.on_starting(t)?;
        let mut rx = client.start_upload(url).await?;
        emitter.on_connected(t, &targets.server_fqdn)?;
        while let Some(result) = rx.recv().await {
            match result {
                Ok(m) => {
                    if !cli.quiet {
                        emitter.on_upload_event(&m)?;
                    }
                    if m.origin == Some(Origin::Server) {
                        ul_measurement = Some(m);
                    }
                }
                Err(e) => emitter.on_error(t, &e.to_string())?,
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
