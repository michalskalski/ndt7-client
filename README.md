# ndt7-client

A Rust client library and CLI for [ndt7](https://github.com/m-lab/ndt-server/blob/master/spec/ndt7-protocol.md), the network speed test protocol developed by [M-Lab](https://www.measurementlab.net/).

ndt7 measures download and upload throughput using WebSocket connections to M-Lab's global server infrastructure, providing TCP-level metrics (latency, retransmission) alongside application-level throughput.

## Library usage

```rust
use ndt7_client::client::ClientBuilder;
use ndt7_client::spec::Origin;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ClientBuilder::new("my-app", "0.1.0").build();

    // Locate the nearest M-Lab server
    let targets = client.locate_test_targets().await?;

    // Run download test
    if let Some(url) = &targets.download_url {
        let mut rx = client.start_download(url).await?;
        while let Some(result) = rx.recv().await {
            let m = result?;
            if m.origin == Some(Origin::Client) {
                if let Some(app) = &m.app_info {
                    let mbps = 8.0 * app.num_bytes as f64 / app.elapsed_time as f64;
                    println!("Download: {mbps:.1} Mbit/s");
                }
            }
        }
    }

    // Run upload test
    if let Some(url) = &targets.upload_url {
        let mut rx = client.start_upload(url).await?;
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
    }

    Ok(())
}
```

## CLI usage

Install:

```sh
cargo install ndt7-client
```

Run a speed test:

```sh
ndt7-client
```

Options:

```
--server <host:port>   Use a specific server (bypass locate API)
--service-url <url>    Full service URL with access token
--format human|json    Output format (default: human)
--no-download          Skip download test
--no-upload            Skip upload test
--quiet                Show summary only
--no-verify            Skip TLS certificate verification
--no-tls               Use unencrypted WebSocket
```

## References

- [M-Lab](https://www.measurementlab.net/) - Measurement Lab
- [ndt7 protocol spec](https://github.com/m-lab/ndt-server/blob/master/spec/ndt7-protocol.md)
- [ndt7-client-go](https://github.com/m-lab/ndt7-client-go) - Go reference implementation

## License

Apache-2.0
