use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{HostWithPortArgs, StartedResponse};

impl RpcCommandHandler {
    pub(crate) fn keepalive(&self, args: HostWithPortArgs) -> anyhow::Result<StartedResponse> {
        self.node
            .rep_crawler
            .keepalive_or_connect(args.address, args.port.into());
        Ok(StartedResponse::new(true))
    }
}
