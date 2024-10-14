use crate::RpcCommand;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn peers(peer_details: Option<bool>) -> Self {
        Self::Peers(PeersArgs::new(peer_details))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PeersArgs {
    pub peer_details: Option<bool>,
}

impl PeersArgs {
    pub fn new(peer_details: Option<bool>) -> Self {
        PeersArgs { peer_details }
    }
}
