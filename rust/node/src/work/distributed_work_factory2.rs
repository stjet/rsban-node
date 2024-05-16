use rsnano_core::{Account, Root};

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

pub struct DistributedWorkFactory2 {}

impl DistributedWorkFactory2 {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn generate_work(&self, request: WorkRequest) -> Option<u64> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn request_one_peer_with_ip_address() {
        let request = WorkRequest {
            peers: vec![("192.168.0.1".to_string(), 5000)],
            ..WorkRequest::create_test_instance()
        };

        let work_factory = DistributedWorkFactory2::new();
        let work = work_factory.generate_work(request).await;
        assert_eq!(work, Some(12345));
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
