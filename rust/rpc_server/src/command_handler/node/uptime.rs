use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::UptimeDto;
use std::time::Instant;

impl RpcCommandHandler {
    pub(crate) fn uptime(&self) -> UptimeDto {
        let seconds = Instant::now() - self.node.telemetry.startup_time;
        UptimeDto::new(seconds.as_secs())
    }
}
