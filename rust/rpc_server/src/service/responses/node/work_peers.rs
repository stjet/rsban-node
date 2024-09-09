use std::sync::Arc;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{ErrorDto, WorkPeersDto};
use serde_json::to_string_pretty;

pub async fn work_peers(node: Arc<Node>, enable_control: bool) -> String {
    if enable_control {
        let work_peers_dto = WorkPeersDto::new(node.config.work_peers.clone());
        to_string_pretty(&work_peers_dto).unwrap()
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
    fn work_peers() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node
            .tokio
            .block_on(async { rpc_client.work_peers().await.unwrap() });

        server.abort();
    }

    #[test]
    fn work_peers_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node
            .tokio
            .block_on(async { rpc_client.work_peers().await });

        assert!(result.is_err());

        server.abort();
    }
}