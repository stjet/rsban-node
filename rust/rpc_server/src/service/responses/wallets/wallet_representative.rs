use rsnano_core::WalletId;
use rsnano_node::Node;
use rsnano_rpc_messages::{ErrorDto, RpcDto, WalletRepresentativeDto};
use std::sync::Arc;

pub async fn wallet_representative(node: Arc<Node>, wallet: WalletId) -> RpcDto {
    match node.wallets.get_representative(wallet) {
        Ok(representative) => RpcDto::WalletRepresentative(WalletRepresentativeDto::new(representative.into())),
        Err(e) => RpcDto::Error(ErrorDto::WalletsError(e))
    }
}
