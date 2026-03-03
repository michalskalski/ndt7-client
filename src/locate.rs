//! M-Lab Locate API client.
//!
//! The Locate API returns the nearest M-Lab servers with signed WebSocket
//! URLs for running ndt7 tests.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Base URL for the M-Lab Locate v2 API.
pub const LOCATE_URL: &str = "https://locate.measurementlab.net/v2/nearest/ndt/ndt7";

/// A single M-Lab server returned by the Locate API.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Target {
    /// FQDN of the server machine.
    pub machine: String,
    /// Map of service key (e.g. `"wss:///ndt/v7/download"`) to full URL with access token.
    pub urls: HashMap<String, String>,
    /// Geographic location of the server, if provided by the API.
    pub location: Option<Location>,
}

/// Download and upload URLs extracted from a [`Target`] for a specific scheme.
pub struct ServiceUrls {
    /// Full URL for the download test, if available.
    pub download: Option<String>,
    /// Full URL for the upload test, if available.
    pub upload: Option<String>,
}

impl Target {
    /// Extract the download and upload URLs for the given scheme (`"wss"` or `"ws"`).
    pub fn service_urls(&self, scheme: &str) -> ServiceUrls {
        let mut dl = None;
        let mut ul = None;
        for (key, url) in &self.urls {
            if key.starts_with(scheme) && key.contains(crate::params::DOWNLOAD_URL_PATH) {
                dl = Some(url.clone());
            } else if key.starts_with(scheme) && key.contains(crate::params::UPLOAD_URL_PATH) {
                ul = Some(url.clone());
            }
        }
        ServiceUrls {
            download: dl,
            upload: ul,
        }
    }
}

/// Geographic location of an M-Lab server.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Location {
    /// City where the server is located (e.g. "Tokyo").
    pub city: String,
    /// Country where the server is located (e.g. "JP").
    pub country: String,
}

/// Top-level response from the Locate API.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct LocateResponse {
    /// Ordered list of nearby servers (closest first).
    pub results: Vec<Target>,
}

/// Query the Locate API for the nearest M-Lab servers.
///
/// Returns [`crate::error::Ndt7Error::NoCapacity`] when the Locate API responds with
/// 204 (M-Lab is out of capacity).
pub async fn nearest(user_agent: &str) -> Result<Vec<Target>> {
    let client = reqwest::Client::builder().user_agent(user_agent).build()?;
    let response = client.get(LOCATE_URL).send().await?.error_for_status()?;

    if response.status() == reqwest::StatusCode::NO_CONTENT {
        return Err(crate::error::Ndt7Error::NoCapacity);
    }

    let locate: LocateResponse = response.json().await?;
    Ok(locate.results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_locate_response() {
        let json = r#"{
           "results": [
               {
                   "machine": "mlab1-lga06.mlab-oss.measurement-lab.org",
                   "urls": {
                       "wss:///ndt/v7/download": "wss://mlab1-lga06:4443/ndt/v7/download?access_token=abc",
                       "wss:///ndt/v7/upload": "wss://mlab1-lga06:4443/ndt/v7/upload?access_token=def"
                   }
               }
           ]
        }"#;

        let l_resp: LocateResponse = serde_json::from_str(json).unwrap();

        let results = l_resp.results;
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].machine,
            "mlab1-lga06.mlab-oss.measurement-lab.org"
        );
        assert_eq!(results[0].urls.len(), 2);
        assert_eq!(results[0].location, None);
    }

    #[test]
    fn deserialize_locate_response_with_location() {
        let json = r#"{
           "results": [
               {
                   "machine": "mlab1-lga06.mlab-oss.measurement-lab.org",
                   "urls": {
                       "wss:///ndt/v7/download": "wss://mlab1-lga06:4443/ndt/v7/download?access_token=abc",
                       "wss:///ndt/v7/upload": "wss://mlab1-lga06:4443/ndt/v7/upload?access_token=def"
                   },
                   "location": {
                       "city": "Tokyo",
                       "country": "JP"
                   }
               }
           ]
        }"#;

        let l_resp: LocateResponse = serde_json::from_str(json).unwrap();

        let results = l_resp.results;
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].machine,
            "mlab1-lga06.mlab-oss.measurement-lab.org"
        );
        assert_eq!(results[0].urls.len(), 2);
        let location = results[0].location.as_ref().unwrap();
        assert_eq!(location.city, "Tokyo");
        assert_eq!(location.country, "JP");
    }

    #[tokio::test]
    #[ignore]
    async fn test_nearest_real_api() {
        let targets = nearest("ndt7-client-rust/test").await.unwrap();
        assert!(!targets.is_empty());
    }
}
