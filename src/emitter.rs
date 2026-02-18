use std::io::Write;

use serde::Serialize;

use crate::error::Result;
use crate::spec::{Measurement, TestKind};
use crate::summary::Summary;

#[derive(Serialize)]
#[serde(tag = "type")]
enum Event<'a> {
    Starting {
        test: TestKind,
    },
    Error {
        test: TestKind,
        error: &'a str,
    },
    Connected {
        test: TestKind,
        fqdn: &'a str,
    },
    Measurement {
        test: TestKind,
        measurement: &'a Measurement,
    },
    Complete {
        test: TestKind,
    },
    Summary {
        summary: &'a Summary,
    },
}

pub trait Emitter {
    fn on_starting(&mut self, test: TestKind) -> Result<()>;
    fn on_error(&mut self, test: TestKind, err: &str) -> Result<()>;
    fn on_connected(&mut self, test: TestKind, fqdn: &str) -> Result<()>;
    fn on_download_event(&mut self, m: &Measurement) -> Result<()>;
    fn on_upload_event(&mut self, m: &Measurement) -> Result<()>;
    fn on_complete(&mut self, test: TestKind) -> Result<()>;
    fn on_summary(&mut self, s: &Summary) -> Result<()>;
}

pub struct HumanReadableEmitter<W: Write> {
    out: W,
}

impl<W: Write> HumanReadableEmitter<W> {
    pub fn new(out: W) -> Self {
        HumanReadableEmitter { out }
    }
}

impl<W: Write> Emitter for HumanReadableEmitter<W> {
    fn on_starting(&mut self, test: TestKind) -> Result<()> {
        write!(self.out, "\rstarting {:?}", test)?;
        Ok(())
    }

    fn on_connected(&mut self, test: TestKind, fqdn: &str) -> Result<()> {
        write!(self.out, "{:?} in progress with {fqdn}", test)?;
        Ok(())
    }

    fn on_error(&mut self, test: TestKind, err: &str) -> Result<()> {
        write!(self.out, "{:?} test failed: {err}", test)?;
        Ok(())
    }

    fn on_complete(&mut self, test: TestKind) -> Result<()> {
        write!(self.out, "{:?}: complete", test)?;
        Ok(())
    }

    fn on_download_event(&mut self, m: &Measurement) -> Result<()> {
        if let Some(app) = &m.app_info {
            if app.elapsed_time > 0 {
                let speed = 8.0 * app.num_bytes as f64 / app.elapsed_time as f64;
                write!(self.out, "\rAvg. speed: {:>7.1} Mbit/s", speed)?;
            }
        }
        Ok(())
    }

    fn on_upload_event(&mut self, m: &Measurement) -> Result<()> {
        if let Some(tcp) = &m.tcp_info {
            if let (Some(received), Some(elapsed)) = (tcp.bytes_received, tcp.elapsed_time) {
                if elapsed > 0 {
                    let speed = 8.0 * received as f64 / elapsed as f64;
                    write!(self.out, "\rAvg. speed: {:>7.1} Mbit/s", speed)?;
                }
            }
        }
        Ok(())
    }

    fn on_summary(&mut self, s: &Summary) -> Result<()> {
        writeln!(self.out, "\nTest results\n")?;
        writeln!(self.out, "{:>10}: {}", "Server", s.server_fqdn)?;
        writeln!(self.out, "{:>10}: {}", "Client", s.client_ip)?;

        if let Some(dl) = &s.download {
            writeln!(self.out, "\n{:>22}", "Download")?;
            writeln!(
                self.out,
                "{:>15}: {:>7.1} Mbit/s",
                "Throughput", dl.throughput_mbps
            )?;
            writeln!(self.out, "{:>15}: {:>7.1} ms", "Latency", dl.latency_ms)?;
            writeln!(
                self.out,
                "{:>15}: {:>7.1} %",
                "Retransmission", dl.retransmission_pct
            )?;
        }

        if let Some(ul) = &s.upload {
            writeln!(self.out, "\n{:>20}", "Upload")?;
            writeln!(
                self.out,
                "{:>15}: {:>7.1} Mbit/s",
                "Throughput", ul.throughput_mbps
            )?;
            writeln!(self.out, "{:>15}: {:>7.1} ms", "Latency", ul.latency_ms)?;
        }

        Ok(())
    }
}

pub struct JsonEmitter<W: Write> {
    out: W,
}

impl<W: Write> JsonEmitter<W> {
    pub fn new(out: W) -> Self {
        JsonEmitter { out }
    }

    fn emit(&mut self, event: &Event) -> Result<()> {
        let json = serde_json::to_string(event)?;
        writeln!(self.out, "{}", json)?;
        Ok(())
    }
}

impl<W: Write> Emitter for JsonEmitter<W> {
    fn on_starting(&mut self, test: TestKind) -> Result<()> {
        self.emit(&Event::Starting { test })
    }

    fn on_error(&mut self, test: TestKind, err: &str) -> Result<()> {
        self.emit(&Event::Error { test, error: err })
    }

    fn on_connected(&mut self, test: TestKind, fqdn: &str) -> Result<()> {
        self.emit(&Event::Connected { test, fqdn })
    }

    fn on_download_event(&mut self, m: &Measurement) -> Result<()> {
        self.emit(&Event::Measurement {
            test: TestKind::Download,
            measurement: m,
        })
    }

    fn on_upload_event(&mut self, m: &Measurement) -> Result<()> {
        self.emit(&Event::Measurement {
            test: TestKind::Upload,
            measurement: m,
        })
    }

    fn on_complete(&mut self, test: TestKind) -> Result<()> {
        self.emit(&Event::Complete { test })
    }

    fn on_summary(&mut self, s: &Summary) -> Result<()> {
        self.emit(&Event::Summary { summary: s })
    }
}

#[cfg(test)]
mod tests {
    use crate::spec::AppInfo;

    use super::*;

    #[test]
    fn human_readable_download_event() {
        let mut buf = Vec::new();
        let mut emitter = HumanReadableEmitter::new(&mut buf);

        let m = Measurement {
            app_info: Some(AppInfo {
                num_bytes: 1_000_000,
                elapsed_time: 1_000_000,
            }),
            ..Default::default()
        };

        emitter.on_download_event(&m).unwrap();

        let out = String::from_utf8(buf).unwrap();

        assert!(out.contains("8.0 Mbit/s"))
    }

    #[test]
    fn json_emitter_valid() {
        let mut buf = Vec::new();
        let mut emitter = JsonEmitter::new(&mut buf);

        emitter.on_starting(TestKind::Upload).unwrap();

        let out = String::from_utf8(buf).unwrap();

        let res = serde_json::from_str::<serde_json::Value>(&out).unwrap();

        assert_eq!(res["test"], "upload");
        assert_eq!(res["type"], "Starting");
    }

}
