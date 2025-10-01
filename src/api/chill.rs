use crate::models::TorrentResult;
use std::error::Error;

#[derive(Clone)]
pub struct ChillClient {
    api_key: String,
    putio_token: Option<String>,
    base_url: String,
}

impl ChillClient {
    pub fn new(api_key: String, putio_token: Option<String>) -> Self {
        Self {
            api_key,
            putio_token,
            base_url: "https://chill.institute/api/v3".to_string(),
        }
    }

    pub fn search(&self, query: &str, indexers: Option<&[String]>, filter_nsfw: bool) -> Result<Vec<TorrentResult>, Box<dyn Error>> {
        let mut url = format!("{}/search?keyword={}", self.base_url, urlencode(query));

        if let Some(idxs) = indexers {
            if !idxs.is_empty() && !idxs.contains(&"all".to_string()) {
                url.push_str(&format!("&indexer={}", idxs.join(",")));
            }
        }

        // Add NSFW filter parameter
        url.push_str(&format!("&filterNastyResults={}", filter_nsfw));

        let mut request = ureq::get(&url)
            .set("Authorization", &self.api_key);

        // Add X-Putio-Token header if available
        if let Some(ref token) = self.putio_token {
            request = request.set("X-Putio-Token", token);
        }

        let response = request.call()?;

        let results: Vec<TorrentResult> = serde_json::from_reader(response.into_reader())?;
        Ok(results)
    }
}

fn urlencode(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            _ => {
                result.push('%');
                result.push_str(&format!("{:02X}", byte));
            }
        }
    }
    result
}