use std::sync::Arc;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{ErrorDto, SuccessDto};
use serde_json::to_string_pretty;

pub async fn work_peers_clear(node: Arc<Node>, enable_control: bool) -> String {
    if enable_control {
        node.config.lock().unwrap().work_peers.clear();
        to_string_pretty(&SuccessDto::new()).unwrap()
    }
    else {
        to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv6Addr;
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_node::config::Peer;
    use test_helpers::System;

    #[test]
    fn work_peers_clear() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        {
            let mut config = node.config.lock().unwrap();
            config.work_peers.push(Peer::new(Ipv6Addr::LOCALHOST.to_string(), 54000));
            assert_eq!(!config.work_peers.is_empty(), true);
        }

        let result = node
            .tokio
            .block_on(async { rpc_client.work_peers_clear().await.unwrap() });

        assert_eq!(node.config.lock().unwrap().work_peers.is_empty(), true);

        server.abort();
    }

    #[test]
    fn work_peers_clear_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node
            .tokio
            .block_on(async { rpc_client.work_peers_clear().await });

        assert!(result.is_err());

        server.abort();
    }
}