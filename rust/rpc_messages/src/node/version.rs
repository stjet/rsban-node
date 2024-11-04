use rsnano_core::BlockHash;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct VersionResponse {
    pub rpc_version: u8,
    pub store_version: i32,
    pub protocol_version: u8,
    pub node_vendor: String,
    pub store_vendor: String,
    pub network: String,
    pub network_identifier: BlockHash,
    pub build_info: String,
}
