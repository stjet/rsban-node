use reqwest::Url;
use rsnano_core::{to_hex_string, Account, Root};
use serde::Serialize;

use super::http_client::HttpClient;

#[derive(Serialize)]
pub struct HttpWorkRequest {
    action: &'static str,
    hash: String,
    difficulty: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    account: Option<String>,
}

#[derive(Clone)]
pub struct WorkRequest {
    pub root: Root,
    pub difficulty: u64,
    pub account: Option<Account>,
    pub peers: Vec<(String, u16)>,
}

impl WorkRequest {
    pub fn create_test_instance() -> Self {
        Self {
            root: Root::from(100),
            difficulty: 42,
            account: Some(Account::from(200)),
            peers: vec![("127.0.0.1".to_string(), 9999)],
        }
    }
}

pub struct DistributedWorkFactory2 {
    http_client: HttpClient,
}

impl DistributedWorkFactory2 {
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
    }

    pub async fn generate_work(&self, request: WorkRequest) -> Option<u64> {
        let (ip, port) = &request.peers[0];
        let url: Url = format!("http://{}:{}/", ip, port).parse().ok()?;
        let request = HttpWorkRequest {
            action: "work_generate",
            hash: request.root.to_string(),
            difficulty: to_hex_string(request.difficulty),
            account: request.account.map(|a| a.encode_account()),
        };
        let result = self.http_client.post_json(url, &request).await.ok()?;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::work::http_client::{ConfiguredResponse, HttpClient};
    use reqwest::StatusCode;
    use rsnano_core::to_hex_string;
    use serde_json::json;

    #[tokio::test]
    async fn request_one_peer_with_ip_address() {
        let http_client =
            HttpClient::null_builder().respond(ConfiguredResponse::new(StatusCode::OK, "TODO"));
        let request_tracker = http_client.track_requests();
        let work_factory = DistributedWorkFactory2::new(http_client);

        let request = WorkRequest {
            peers: vec![("192.168.0.1".to_string(), 5000)],
            ..WorkRequest::create_test_instance()
        };

        let work = work_factory.generate_work(request.clone()).await;

        let requests = request_tracker.output();
        assert_eq!(requests.len(), 1, "no request sent");

        let (url, content) = &requests[0];
        assert_eq!(url.to_string(), "http://192.168.0.1:5000/");

        let expected_message = json!({
            "action": "work_generate",
            "hash": request.root.to_string(),
            "difficulty": to_hex_string(request.difficulty),
            "account": request.account.unwrap().encode_account()
        });
        assert_eq!(*content, serde_json::to_value(&expected_message).unwrap());
    }

    // TODO:
    // Backoff + Workrequest
    // Cancel
    // Local work
    // resolve hostnames
    // multiple peers
    // secondary peers
    // work generation disabled
    // unresponsive work peers => use local work
}
