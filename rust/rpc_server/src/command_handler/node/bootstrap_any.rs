use crate::command_handler::RpcCommandHandler;
use rsnano_node::bootstrap::BootstrapInitiatorExt;
use rsnano_rpc_messages::{BootstrapAnyArgs, ErrorDto, RpcDto, SuccessDto};

impl RpcCommandHandler {
    pub(crate) fn bootstrap_any(&self, args: BootstrapAnyArgs) -> RpcDto {
        if self.node.flags.disable_legacy_bootstrap {
            return RpcDto::Error(ErrorDto::LegacyBootstrapDisabled);
        }

        let force = args.force.unwrap_or(false);
        let bootstrap_id = args.id.unwrap_or_default();
        let start_account = args.account.unwrap_or_default();

        self.node
            .bootstrap_initiator
            .bootstrap(force, bootstrap_id, u32::MAX, start_account);

        RpcDto::BootstrapAny(SuccessDto::new())
    }
}
