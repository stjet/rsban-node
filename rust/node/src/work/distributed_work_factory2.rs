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

impl HttpWorkRequest {
    pub fn new(root: Root, difficulty: u64, account: Option<Account>) -> Self {
        Self {
            action: "work_generate",
            hash: root.to_string(),
            difficulty: to_hex_string(difficulty),
            account: account.map(|a| a.encode_account()),
        }
    }
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
            self.generate_in_local_work_pool(request.root, request.difficulty)
                .await
        } else {
            let (host, port) = request.peers.first().unwrap();
            self.generate_remote(
                host,
                *port,
                request.root,
                request.difficulty,
                request.account,
            )
            .await
        }
    }

    async fn generate_in_local_work_pool(&self, root: Root, difficulty: u64) -> Option<u64> {
        let (tx, rx) = oneshot::channel::<Option<u64>>();
        self.work_pool.generate_async(
            WorkVersion::Work1,
            root,
            difficulty,
            Some(Box::new(move |work| {
                tx.send(work).unwrap();
            })),
        );
        rx.await.ok()?
    }

    async fn generate_remote(
        &self,
        host: &str,
        port: u16,
        root: Root,
        difficulty: u64,
        account: Option<Account>,
    ) -> Option<u64> {
        let url = Self::remote_url(host, port).ok()?;
        let request = HttpWorkRequest::new(root, difficulty, account);

        let result = self.http_client.post_json(url, &request).await.ok()?;
        //TODO check status code
        let work_response: HttpWorkResponse = result.json().await.ok()?;
        u64_from_hex_str(&work_response.work).ok()
    }

    fn remote_url(host: &str, port: u16) -> anyhow::Result<Url> {
        let url = format!("http://{}:{}/", host, port).parse::<Url>()?;
        Ok(url)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::work::http_client::{ConfiguredResponse, HttpClient};
    use reqwest::{Method, StatusCode};
    use rsnano_core::{to_hex_string, work::WorkPoolImpl};
    use serde_json::json;

    #[tokio::test]
    async fn request_one_peer_with_ip_address() {
        let expected_work = 12345;
        let work_pool = Arc::new(WorkPoolImpl::new_null(0));
        let http_client = HttpClient::null_builder().respond(ConfiguredResponse::new(
            StatusCode::OK,
            HttpWorkResponse {
                work: to_hex_string(expected_work),
            },
        ));
        let request_tracker = http_client.track_requests();
        let work_factory = DistributedWorkFactory2::new(http_client, work_pool);

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
        let work_factory = DistributedWorkFactory2::new(http_client, work_pool);

        let request = WorkRequest {
            peers: vec![],
            ..WorkRequest::create_test_instance()
        };

        let work = work_factory.generate_work(request.clone()).await;

        assert_eq!(work, Some(expected_work));
        assert_eq!(
            request_tracker.output().len(),
            0,
            "no request should be sent!"
        );
    }

    #[tokio::test]
    async fn request_multiple_peers_concurrently() {
        let expected_work = 12345;
        let work_pool = Arc::new(WorkPoolImpl::new_null(0));
        let http_client = HttpClient::null_builder()
            .respond_url(
                Method::POST,
                "http://192.168.0.1:3000/",
                ConfiguredResponse::freeze(),
            )
            .respond_url(
                Method::POST,
                "http://192.168.0.2:3000/",
                ConfiguredResponse::freeze(),
            )
            .respond_url(
                Method::POST,
                "http://192.168.0.3:3000/",
                ConfiguredResponse::new(
                    StatusCode::OK,
                    HttpWorkResponse {
                        work: to_hex_string(expected_work),
                    },
                ),
            )
            .finish();
        let request_tracker = http_client.track_requests();
        let work_factory = DistributedWorkFactory2::new(http_client, work_pool);

        let request = WorkRequest {
            peers: vec![
                ("192.168.0.1".to_string(), 3000),
                ("192.168.0.2".to_string(), 3000),
                ("192.168.0.3".to_string(), 3000),
            ],
            ..WorkRequest::create_test_instance()
        };

        let work = work_factory.generate_work(request.clone()).await;

        let requests = request_tracker.output();
        assert_eq!(requests.len(), 3);
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
