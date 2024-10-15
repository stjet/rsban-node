use rsnano_core::WalletId;
use rsnano_node::{wallets::WalletsExt, Node};
use rsnano_rpc_messages::{ErrorDto, ExistsDto, RpcDto};
use std::sync::Arc;

pub async fn search_receivable(node: Arc<Node>, enable_control: bool, wallet: WalletId) -> RpcDto {
    if enable_control {
        match node.wallets.search_receivable_wallet(wallet) {
            Ok(_) => RpcDto::SearchReceivable(ExistsDto::new(true)),
            Err(e) => RpcDto::Error(ErrorDto::WalletsError(e))
        }
    } else {
        RpcDto::Error(ErrorDto::RPCControlDisabled)
    }
}
