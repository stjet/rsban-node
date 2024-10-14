use rsnano_node::Node;
use rsnano_rpc_messages::{AccountRemoveArgs, ErrorDto2, RemovedDto, RpcDto};
use std::sync::Arc;

pub async fn account_remove(
    node: Arc<Node>,
    enable_control: bool,
    args: AccountRemoveArgs,
) -> RpcDto {
    if enable_control {
        match node.wallets.remove_key(&args.wallet, &args.account.into()) {
            Ok(()) => RpcDto::Removed(RemovedDto::new(true)),
            Err(e) => RpcDto::Error(ErrorDto2::WalletsError(e)),
        }
    } else {
        RpcDto::Error(ErrorDto2::RPCControlDisabled)
    }
}
