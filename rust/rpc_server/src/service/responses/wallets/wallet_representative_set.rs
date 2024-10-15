use rsnano_core::{Account, WalletId};
use rsnano_node::{wallets::WalletsExt, Node};
use rsnano_rpc_messages::{ErrorDto, RpcDto, SetDto};
use std::sync::Arc;

pub async fn wallet_representative_set(
    node: Arc<Node>,
    enable_control: bool,
    wallet_id: WalletId,
    representative: Account,
    update_existing_accounts: Option<bool>,
) -> RpcDto {
    if enable_control {
        let update_existing = update_existing_accounts.unwrap_or(false);
        match node
            .wallets
            .set_representative(wallet_id, representative.into(), update_existing)
        {
            Ok(_) => RpcDto::WalletRepresentativeSet(SetDto::new(true)),
            Err(e) => RpcDto::Error(ErrorDto::WalletsError(e))
        }
    } else {
        RpcDto::Error(ErrorDto::RPCControlDisabled)
    }
}
