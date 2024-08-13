mod node;
mod wallet;

pub(crate) use node::*;
use serde::Deserialize;
pub(crate) use wallet::*;

#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum RpcRequest {
    Node(NodeRpcRequest),
    Wallet(WalletRpcRequest),
}
