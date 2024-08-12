mod node;
mod wallet;

pub(crate) use node::*;
use serde::Deserialize;
pub(crate) use wallet::*;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub(crate) enum RpcRequest {
    Node(NodeRpcRequest),
    Wallet(WalletRpcRequest),
}
