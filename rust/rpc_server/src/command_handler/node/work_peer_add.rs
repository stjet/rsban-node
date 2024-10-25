use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{AddressWithPortArgs, ErrorDto, RpcDto};
use tracing::warn;

impl RpcCommandHandler {
    pub(crate) fn work_peer_add(&self, _args: AddressWithPortArgs) -> RpcDto {
        warn!("Distributed work feature is not implemented yet");
        RpcDto::Error(ErrorDto::Other)
    }
}
