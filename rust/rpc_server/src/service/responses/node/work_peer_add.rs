use std::{net::Ipv6Addr, sync::Arc};
use rsnano_node::{config::Peer, node::Node};
use rsnano_rpc_messages::{ErrorDto, SuccessDto};
use serde_json::to_string_pretty;

pub async fn work_peer_add(node: Arc<Node>, enable_control: bool, address: Ipv6Addr, port: u16) -> String {
    if enable_control {
        node.config.lock().unwrap().work_peers.push(Peer::new(address.to_string(), port));
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
    use test_helpers::{get_available_port, System};

    #[test]
    fn work_peer_add() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let (address, port) = (Ipv6Addr::LOCALHOST, get_available_port());

        node
            .tokio
            .block_on(async { rpc_client.work_peer_add(address, port).await.unwrap() });

        assert!(node.config.lock().unwrap().work_peers.contains(&Peer::new(address.to_string(), port)));

        server.abort();
    }

    #[test]
    fn work_peer_add_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node
            .tokio
            .block_on(async { rpc_client.work_peer_add(Ipv6Addr::LOCALHOST, get_available_port()).await });

        assert!(result.is_err());

        server.abort();
    }
}