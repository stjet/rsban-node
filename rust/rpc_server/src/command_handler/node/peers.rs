use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{PeerData, PeerInfo, PeersArgs, PeersDto};
use std::collections::HashMap;

impl RpcCommandHandler {
    pub(crate) fn peers(&self, args: PeersArgs) -> PeersDto {
        let peer_details = args.peer_details.unwrap_or(false);
        let mut peers: HashMap<String, PeerInfo> = HashMap::new();

        self.node
            .network_info
            .read()
            .unwrap()
            .random_realtime_channels(usize::MAX, 0)
            .iter()
            .for_each(|channel| {
                peers.insert(
                    channel.ipv4_address_or_ipv6_subnet().to_string(),
                    PeerInfo::Detailed {
                        protocol_version: channel.protocol_version(),
                        node_id: channel.node_id().unwrap().into(),
                        connection_type: "tcp".to_string(),
                    },
                );
            });

        let peer_data = if peer_details {
            PeerData::Detailed(peers)
        } else {
            PeerData::Simple(peers.keys().cloned().collect())
        };

        PeersDto::new(peer_data)
    }
}
