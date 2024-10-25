use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ErrorDto, NodeIdDto, RpcDto};

impl RpcCommandHandler {
    pub(crate) fn node_id(&self) -> RpcDto {
        if self.enable_control {
            let private = self.node.node_id.private_key();
            let public = self.node.node_id.public_key();
            let as_account = public.as_account();

            RpcDto::NodeId(NodeIdDto::new(private, public, as_account))
        } else {
            RpcDto::Error(ErrorDto::RPCControlDisabled)
        }
    }
}
