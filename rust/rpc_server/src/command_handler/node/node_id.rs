use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::NodeIdDto;

impl RpcCommandHandler {
    pub(crate) fn node_id(&self) -> NodeIdDto {
        let private = self.node.node_id.private_key();
        let public = self.node.node_id.public_key();
        let as_account = public.as_account();
        NodeIdDto::new(private, public, as_account)
    }
}
