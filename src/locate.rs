//! M-Lab Locate API client.
//!
//! The Locate API returns the nearest M-Lab servers with signed WebSocket
//! URLs for running ndt7 tests.

use crate::error::Result;
use serde::Deserialize;
use std::collections::HashMap;

/// Base URL for the M-Lab Locate v2 API.
pub const LOCATE_URL: &str = "https://locate.measurementlab.net/v2/nearest/ndt/ndt7";

/// A single M-Lab server returned by the Locate API.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Target {
    /// FQDN of the server machine.
    pub machine: String,
    /// Map of service key (e.g. `"wss:///ndt/v7/download"`) to full URL with access token.
    pub urls: HashMap<String, String>,
}

/// Top-level response from the Locate API.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LocateResponse {
    /// Ordered list of nearby servers (closest first).
    pub results: Vec<Target>,
}

/// Query the Locate API for the nearest M-Lab servers.
///
/// Returns [`Ndt7Error::NoCapacity`] when the Locate API responds with
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
    }

    #[tokio::test]
    #[ignore]
    async fn test_nearest_real_api() {
        let targets = nearest("ndt7-client-rust/test").await.unwrap();
        assert!(!targets.is_empty());
    }
}
