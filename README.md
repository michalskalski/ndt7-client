# ndt7-client

[![CI](https://github.com/michalskalski/ndt7-client/actions/workflows/ci.yml/badge.svg)](https://github.com/michalskalski/ndt7-client/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/ndt7-client.svg)](https://crates.io/crates/ndt7-client)

A Rust client library and CLI for [ndt7](https://github.com/m-lab/ndt-server/blob/master/spec/ndt7-protocol.md), the network speed test protocol developed by [M-Lab](https://www.measurementlab.net/).

ndt7 measures download and upload throughput using WebSocket connections to M-Lab's global server infrastructure, providing TCP-level metrics (latency, retransmission) alongside application-level throughput.

## Library usage

```rust
use ndt7_client::client::ClientBuilder;
use ndt7_client::spec::Origin;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ClientBuilder::new("my-app", "0.1.0").build();

    // Run download test (auto-locates nearest M-Lab server with retry)
    let handle = client.start_download(None).await?;
    println!("Connected to {}", handle.server_fqdn);
    let mut rx = handle.rx;
    while let Some(result) = rx.recv().await {
        let m = result?;
        if m.origin == Some(Origin::Client) {
            if let Some(app) = &m.app_info {
                let mbps = 8.0 * app.num_bytes as f64 / app.elapsed_time as f64;
                println!("Download: {mbps:.1} Mbit/s");
            }
        }
    }

    // Run upload test (reuses cached server list)
    let handle = client.start_upload(None).await?;
    let mut rx = handle.rx;
    while let Some(result) = rx.recv().await {
        let m = result?;
        if m.origin == Some(Origin::Server) {
            if let Some(tcp) = &m.tcp_info {
                if let (Some(received), Some(elapsed)) =
                    (tcp.bytes_received, tcp.elapsed_time)
                {
                    let mbps = 8.0 * received as f64 / elapsed as f64;
                    println!("Upload: {mbps:.1} Mbit/s");
                }
            }
        }
    }

    Ok(())
}
```

## CLI usage

Install:

Pre-built binaries for Linux, macOS, Windows, and illumos are available on the
[GitHub releases page](https://github.com/michalskalski/ndt7-client/releases).

Or install from crates.io:

```console
cargo install ndt7-client
```

Run a speed test:

```console
$ ndt7-client
Download in progress with mlab2-hnd02.mlab-oti.measurement-lab.org
Avg. speed:  1456.0 Mbit/s
Download: complete
Upload in progress with mlab2-hnd02.mlab-oti.measurement-lab.org
Avg. speed:  1734.5 Mbit/s
Upload: complete

Test results

    Server: mlab2-hnd02.mlab-oti.measurement-lab.org
    Client: 2001:db8::1

              Download
     Throughput:  1456.0 Mbit/s
        Latency:     3.0 ms
 Retransmission:     0.5 %

              Upload
     Throughput:  1734.5 Mbit/s
        Latency:     3.3 ms
```

Options:

```
--server [<SERVER>]          Server hostname. With --no-locate: connect directly (e.g. localhost:8080). Without --no-locate: select this server via locate API (gets access tokens). With no value: interactive server picker
--service-url <SERVICE_URL>  Full service URL with path and access token. For advanced use / scripting
--no-locate                  Skip locate API, connect directly to the server specified by --server
--no-tls                     Use unencrypted WebSocket (ws://) instead of TLS (wss://)
--format <FORMAT>            Output format to use: 'human' or 'json' for batch processing [default: human] [possible values: human, json]
--no-download                Skip download measurement
--no-upload                  Skip upload measurement
--quiet                      Emit summary and errors only
--no-verify                  Skip tls certificate verification
--list-servers               List available target servers and exit
--ipv4                       Force IPv4 connections
--ipv6                       Force IPv6 connections
--help                       Print help
```

## References

- [M-Lab](https://www.measurementlab.net/) - Measurement Lab
- [ndt7 protocol spec](https://github.com/m-lab/ndt-server/blob/master/spec/ndt7-protocol.md)
- [ndt7-client-go](https://github.com/m-lab/ndt7-client-go) - Go reference implementation

## License

Apache-2.0
