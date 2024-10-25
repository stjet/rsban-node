use rsnano_node::Node;
use rsnano_rpc_messages::{AddressWithPortArgs, ErrorDto, RpcDto};
use std::sync::Arc;
use tracing::warn;

pub async fn work_peer_add(
    _node: Arc<Node>,
    _enable_control: bool,
    _args: AddressWithPortArgs,
) -> RpcDto {
    warn!("Distributed work feature is not implemented yet");
    RpcDto::Error(ErrorDto::Other)
}
