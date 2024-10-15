use rsnano_node::Node;
use rsnano_rpc_messages::{DestroyedDto, ErrorDto2, RpcDto, WalletDestroyArgs};
use std::sync::Arc;

pub async fn wallet_destroy(node: Arc<Node>, enable_control: bool, args: WalletDestroyArgs) -> RpcDto {
    if enable_control {
        node.wallets.destroy(&args.wallet);
        RpcDto::Destroyed(DestroyedDto::new(true))
    } else {
        RpcDto::Error(ErrorDto2::RPCControlDisabled)
    }
}
