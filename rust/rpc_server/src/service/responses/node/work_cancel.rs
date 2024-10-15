use rsnano_core::BlockHash;
use rsnano_node::Node;
use rsnano_rpc_messages::{ErrorDto, RpcDto, SuccessDto};
use std::sync::Arc;

pub async fn work_cancel(node: Arc<Node>, enable_control: bool, hash: BlockHash) -> RpcDto {
    if enable_control {
        node.distributed_work.cancel(hash.into());
        RpcDto::WorkCancel(SuccessDto::new())
    } else {
        RpcDto::Error(ErrorDto::RPCControlDisabled)
    }
}
