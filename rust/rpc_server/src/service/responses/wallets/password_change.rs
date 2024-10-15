use rsnano_core::WalletId;
use rsnano_node::Node;
use rsnano_rpc_messages::{ErrorDto, RpcDto, SuccessDto};
use std::sync::Arc;

pub async fn password_change(
    node: Arc<Node>,
    enable_control: bool,
    wallet: WalletId,
    password: String,
) -> RpcDto {
    if enable_control {
        match node.wallets.rekey(&wallet, password) {
            Ok(_) => RpcDto::PasswordChange(SuccessDto::new()),
            Err(e) => RpcDto::Error(ErrorDto::WalletsError(e))
        }
    } else {
        RpcDto::Error(ErrorDto::RPCControlDisabled)
    }
}
