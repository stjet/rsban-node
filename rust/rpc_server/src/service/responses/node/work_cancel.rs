use std::sync::Arc;
use rsnano_core::BlockHash;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{ErrorDto, SuccessDto};
use serde_json::to_string_pretty;

pub async fn work_cancel(node: Arc<Node>, enable_control: bool, hash: BlockHash) -> String {
    if enable_control {
        node.distributed_work.cancel(hash.into()); 
        to_string_pretty(&SuccessDto::new()).unwrap()
    }
    else {
        to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::BlockHash;
    use rsnano_node::work::WorkRequest;
    use test_helpers::System;
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;

    #[tokio::test]
    async fn work_cancel() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);
        
        // Start work generation
        let work_handle = tokio::spawn({
            let node = node.clone();
            async move {
                node.distributed_work.generate_work(WorkRequest::new_test_instance()).await
            }
        });

        // Give some time for work generation to start
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Cancel the work
        let result = rpc_client.work_cancel(WorkRequest::new_test_instance().root.into()).await.unwrap();

        // Check the result
        assert_eq!(result, SuccessDto::new());

        // Ensure work generation was actually cancelled
        tokio::time::timeout(std::time::Duration::from_secs(1), work_handle).await
            .expect_err("Work generation should have been cancelled");

        server.abort();
    }
}