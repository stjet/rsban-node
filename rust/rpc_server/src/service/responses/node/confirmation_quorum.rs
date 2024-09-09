use std::sync::Arc;
use rsnano_core::utils::NULL_ENDPOINT;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{ConfirmationQuorumDto, PeerDetailsDto};
use serde_json::to_string_pretty;

pub async fn confirmation_quorum(node: Arc<Node>, peer_details: Option<bool>) -> String {
    let quorum = node.online_reps.lock().unwrap();

    let mut confirmation_quorum_dto = ConfirmationQuorumDto {
        quorum_delta: quorum.quorum_delta(),
        online_weight_quorum_percent: quorum.quorum_percent(),
        online_weight_minimum: quorum.online_weight_minimum(),
        online_stake_total: quorum.online_weight(),
        trended_stake_total: quorum.trended_weight(),
        peers_stake_total: quorum.peered_weight(),
        peers: None,
    };

    if peer_details.unwrap_or(false) {
        let peers = quorum.peered_reps().iter().map(|rep| {
            let endpoint = node
                .network_info
                .read()
                .unwrap()
                .get(rep.channel_id)
                .map(|c| c.peer_addr())
                .unwrap_or(NULL_ENDPOINT);

            PeerDetailsDto {
                account: rep.account.into(),
                ip: endpoint,
                weight: node.ledger.weight(&rep.account),
            }
        }).collect();

        confirmation_quorum_dto.peers = Some(peers);
    }

    to_string_pretty(&confirmation_quorum_dto).unwrap()
}

#[cfg(test)]
mod tests {
    use test_helpers::System;
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;

    #[test]
    fn confirmation_quorum() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node.tokio.block_on(async {
            rpc_client
                .confirmation_quorum(None)
                .await
                .unwrap()
        });

        server.abort();
    }
}