use std::{collections::HashMap, sync::Arc};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{PeerData, PeerInfo, PeersDto};

pub async fn peers(node: Arc<Node>, peer_details: Option<bool>) -> String {
    let peer_details = peer_details.unwrap_or(false);
    let mut peers: HashMap<String, PeerInfo> = HashMap::new();
  
    node.network_info.read()
    .unwrap()
    .random_realtime_channels(usize::MAX, 0)
    .iter()
    .for_each(|channel| {
        peers.insert(
            channel.ipv4_address_or_ipv6_subnet().to_string(),
            PeerInfo::Detailed {
                protocol_version: channel.node_id().unwrap_or_default().to_node_id(),
                node_id: channel.protocol_version().to_string(),
                connection_type: "tcp".to_string(),
            }
        );
    });

    let peer_data = if peer_details {
        PeerData::Detailed(peers)
    } else {
        PeerData::Simple(peers.keys().cloned().collect())
    };

    let peer_dto = PeersDto::new(peer_data);

    serde_json::to_string_pretty(&peer_dto).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use test_helpers::System;

    #[test]
    fn peers_without_details() {
        let mut system = System::new();
        let node1 = system.make_node();
        let node2 = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node1.clone(), false);

        let result = node1.tokio.block_on(async {
            rpc_client.peers(None).await.unwrap()
        });

        match result.peers {
            PeerData::Simple(peers) => {
                assert!(!peers.is_empty());
                // Add more specific assertions for Simple peer data
            },
            PeerData::Detailed(_) => panic!("Expected Simple peer data"),
        }

        server.abort();
    }

    #[test]
    fn peers_with_details() {
        let mut system = System::new();
        let node1 = system.make_node();
        let node2 = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node1.clone(), false);

        let result = node1.tokio.block_on(async {
            rpc_client.peers(Some(true)).await.unwrap()
        });

        match result.peers {
            PeerData::Detailed(peers) => {
                assert!(!peers.is_empty());
                // Add more specific assertions for Detailed peer data
            },
            PeerData::Simple(_) => panic!("Expected Detailed peer data"),
        }

        server.abort();
    }

    // Additional tests can be added here
}