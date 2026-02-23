use crate::error::Result;
use serde::Deserialize;
use std::collections::HashMap;

pub const LOCATE_URL: &str = "https://locate.measurementlab.net/v2/nearest/ndt/ndt7";

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Target {
    pub machine: String,
    pub urls: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LocateResponse {
    pub results: Vec<Target>,
}

pub async fn nearest(user_agent: &str) -> Result<Vec<Target>> {
    let client = reqwest::Client::builder().user_agent(user_agent).build()?;
    let response: LocateResponse = client.get(LOCATE_URL).send().await?.error_for_status()?.json().await?;
    Ok(response.results)
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
