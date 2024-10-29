use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{HostWithPortArgs, StartedDto};

impl RpcCommandHandler {
    pub(crate) fn keepalive(&self, args: HostWithPortArgs) -> anyhow::Result<StartedDto> {
        self.node
            .rep_crawler
            .keepalive_or_connect(args.address, args.port);
        Ok(StartedDto::new(true))
    }
}
