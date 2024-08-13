mod node;
mod wallet;

pub use node::*;
use serde::Deserialize;
pub use wallet::*;

#[derive(Deserialize)]
#[serde(untagged)]
pub enum RpcRequest {
    Node(NodeRpcRequest),
    Wallet(WalletRpcRequest),
}
