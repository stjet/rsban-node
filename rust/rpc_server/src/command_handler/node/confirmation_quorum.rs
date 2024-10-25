use crate::command_handler::RpcCommandHandler;
use rsnano_core::utils::NULL_ENDPOINT;
use rsnano_rpc_messages::{ConfirmationQuorumArgs, ConfirmationQuorumDto, PeerDetailsDto, RpcDto};

impl RpcCommandHandler {
    pub(crate) fn confirmation_quorum(&self, args: ConfirmationQuorumArgs) -> RpcDto {
        let quorum = self.node.online_reps.lock().unwrap();

        let mut confirmation_quorum_dto = ConfirmationQuorumDto {
            quorum_delta: quorum.quorum_delta(),
            online_weight_quorum_percent: quorum.quorum_percent(),
            online_weight_minimum: quorum.online_weight_minimum(),
            online_stake_total: quorum.online_weight(),
            trended_stake_total: quorum.trended_weight(),
            peers_stake_total: quorum.peered_weight(),
            peers: None,
        };

        if args.peer_details.unwrap_or(false) {
            let peers = quorum
                .peered_reps()
                .iter()
                .map(|rep| {
                    let endpoint = self
                        .node
                        .network_info
                        .read()
                        .unwrap()
                        .get(rep.channel_id)
                        .map(|c| c.peer_addr())
                        .unwrap_or(NULL_ENDPOINT);

                    PeerDetailsDto {
                        account: rep.account.into(),
                        ip: endpoint,
                        weight: self.node.ledger.weight(&rep.account),
                    }
                })
                .collect();

            confirmation_quorum_dto.peers = Some(peers);
        }

        RpcDto::ConfirmationQuorum(confirmation_quorum_dto)
    }
}
