mod ledger;
mod node;
mod utils;
mod wallets;

pub use ledger::*;
pub use node::*;
use serde::{Deserialize, Serialize};
pub use utils::*;
pub use wallets::*;

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcCommand {
    Ledger(LedgerRpcCommand),
    Node(NodeRpcCommand),
    Utils(UtilsRpcCommand),
    Wallets(WalletsRpcCommand),
}
