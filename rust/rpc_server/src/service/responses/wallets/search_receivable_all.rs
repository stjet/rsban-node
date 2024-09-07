use std::sync::Arc;
use rsnano_node::node::{Node, NodeExt};
use rsnano_rpc_messages::{ErrorDto, SuccessDto};
use serde_json::to_string_pretty;

pub async fn search_receivable_all(node: Arc<Node>, enable_control: bool) -> String {
    if enable_control {
        node.search_receivable_all();
        to_string_pretty(&SuccessDto::new()).unwrap()
    }
    else {
        to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use test_helpers::System;

    #[test]
    fn search_receivable_all() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        node
            .tokio
            .block_on(async { rpc_client.search_receivable_all().await.unwrap() });

        server.abort();
    }

    #[test]
    fn search_receivable_all_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node
            .tokio
            .block_on(async { rpc_client.search_receivable_all().await });

        assert_eq!(
            result.err().map(|e| e.to_string()),
            Some("node returned error: \"RPC control is disabled\"".to_string())
        );
    
        server.abort();
    }
}