use super::http_client::HttpClient;
use reqwest::Url;
use rsnano_core::{
    to_hex_string, u64_from_hex_str,
    work::{WorkPool, WorkPoolImpl},
    Account, Root, WorkVersion,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::oneshot;

#[derive(Serialize)]
pub struct HttpWorkRequest {
    action: &'static str,
    hash: String,
    difficulty: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    account: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct HttpWorkResponse {
    work: String,
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
    work_pool: Arc<WorkPoolImpl>,
}

impl DistributedWorkFactory2 {
    pub fn new(http_client: HttpClient, work_pool: Arc<WorkPoolImpl>) -> Self {
        Self {
            http_client,
            work_pool,
        }
    }

    pub async fn generate_work(&self, request: WorkRequest) -> Option<u64> {
        if request.peers.is_empty() {
            let (tx, rx) = oneshot::channel::<Option<u64>>();
            self.work_pool.generate_async(
                WorkVersion::Work1,
                request.root,
                request.difficulty,
                Some(Box::new(move |work| {
                    tx.send(work);
                })),
            );
            rx.await.ok()?
        } else {
            let (ip, port) = request.peers.first().unwrap();
            let url: Url = format!("http://{}:{}/", ip, port).parse().ok()?;
            let request = HttpWorkRequest {
                action: "work_generate",
                hash: request.root.to_string(),
                difficulty: to_hex_string(request.difficulty),
                account: request.account.map(|a| a.encode_account()),
            };
            let result = self.http_client.post_json(url, &request).await.ok()?;
            //TODO check status code
            let work_response: HttpWorkResponse = result.json().await.ok()?;
            u64_from_hex_str(&work_response.work).ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::work::http_client::{ConfiguredResponse, HttpClient};
    use reqwest::StatusCode;
    use rsnano_core::{
        to_hex_string,
        work::{StubWorkPool, WorkPoolImpl},
    };
    use serde_json::json;

    #[tokio::test]
    async fn request_one_peer_with_ip_address() {
        let expected_work = 12345;
        let http_client = HttpClient::null_builder().respond(ConfiguredResponse::new(
            StatusCode::OK,
            HttpWorkResponse {
                work: to_hex_string(expected_work),
            },
        ));
        let request_tracker = http_client.track_requests();
        let work_factory = DistributedWorkFactory2::new(http_client);

        let request = WorkRequest {
            peers: vec![("192.168.0.1".to_string(), 5000)],
            ..WorkRequest::create_test_instance()
        };

        let work = work_factory.generate_work(request.clone()).await;

        let requests = request_tracker.output();
        assert_eq!(requests.len(), 1, "no request sent");
        assert_eq!(requests[0].url.to_string(), "http://192.168.0.1:5000/");

        let expected_message = json!({
            "action": "work_generate",
            "hash": request.root.to_string(),
            "difficulty": to_hex_string(request.difficulty),
            "account": request.account.unwrap().encode_account()
        });
        assert_eq!(
            requests[0].json,
            serde_json::to_value(&expected_message).unwrap()
        );

        assert_eq!(work, Some(expected_work));
    }

    #[tokio::test]
    async fn use_local_work_factor_when_no_peers_given() {
        let expected_work = 12345;
        let work_pool = Arc::new(WorkPoolImpl::new_null(expected_work));
        let http_client = HttpClient::new_null();
        let request_tracker = http_client.track_requests();
        let work_factory = DistributedWorkFactory2::new(http_client);

        let request = WorkRequest {
            peers: vec![],
            ..WorkRequest::create_test_instance()
        };

        let work = work_factory.generate_work(request.clone()).await;

        let requests = request_tracker.output();
        assert_eq!(requests.len(), 0, "no request should be sent!");
        assert_eq!(work, Some(expected_work));
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
