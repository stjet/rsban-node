use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{RpcDto, UptimeDto};
use std::time::Instant;

impl RpcCommandHandler {
    pub(crate) fn uptime(&self) -> RpcDto {
        let seconds = Instant::now() - self.node.telemetry.startup_time;
        let uptime = UptimeDto::new(seconds.as_secs());
        RpcDto::Uptime(uptime)
    }
}
