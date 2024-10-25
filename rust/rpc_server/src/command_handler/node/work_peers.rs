use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ErrorDto, RpcDto};
use tracing::warn;

impl RpcCommandHandler {
    pub(crate) fn work_peers(&self) -> RpcDto {
        warn!("Distributed work feature is not implemented yet");
        RpcDto::Error(ErrorDto::Other)
    }
}
