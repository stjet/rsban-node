use rsnano_node::Node;
use rsnano_rpc_messages::{ErrorDto, RpcDto};
use std::sync::Arc;
use tracing::warn;

pub async fn work_peers(_node: Arc<Node>, _enable_control: bool) -> RpcDto {
    warn!("Distributed work feature is not implemented yet");
    RpcDto::Error(ErrorDto::Other)
}
