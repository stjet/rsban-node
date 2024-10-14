use crate::RpcResult;
use rsnano_node::Node;
use rsnano_rpc_messages::{AccountRemoveArgs, ErrorDto2, RemovedDto};
use std::sync::Arc;

pub async fn account_remove(
    node: Arc<Node>,
    enable_control: bool,
    args: AccountRemoveArgs,
) -> RpcResult<RemovedDto> {
    if enable_control {
        match node.wallets.remove_key(&args.wallet, &args.account.into()) {
            Ok(()) => RpcResult::Ok(RemovedDto::new(true)),
            Err(e) => RpcResult::Err(ErrorDto2::WalletsError(e)),
        }
    } else {
        RpcResult::Err(ErrorDto2::RPCControlDisabled)
    }
}
