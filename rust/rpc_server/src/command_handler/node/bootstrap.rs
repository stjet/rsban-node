use crate::command_handler::RpcCommandHandler;
use rsnano_node::bootstrap::BootstrapInitiatorExt;
use rsnano_rpc_messages::{BootstrapArgs, RpcDto, SuccessDto};
use std::net::SocketAddrV6;

impl RpcCommandHandler {
    pub(crate) fn bootstrap(&self, args: BootstrapArgs) -> RpcDto {
        let id = args.id.unwrap_or(String::new());
        let endpoint = SocketAddrV6::new(args.address, args.port, 0, 0);
        self.node.bootstrap_initiator.bootstrap2(endpoint, id);
        RpcDto::Bootstrap(SuccessDto::new())
    }
}
