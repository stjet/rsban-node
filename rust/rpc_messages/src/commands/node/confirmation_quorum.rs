use crate::RpcCommand;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn confirmation_quorum(peer_details: Option<bool>) -> Self {
        Self::ConfirmationQuorum(ConfirmationQuorumArgs::new(peer_details))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ConfirmationQuorumArgs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_details: Option<bool>,
}

impl ConfirmationQuorumArgs {
    pub fn new(peer_details: Option<bool>) -> Self {
        Self { peer_details }
    }
}
