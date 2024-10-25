use crate::command_handler::RpcCommandHandler;
use rsnano_node::bootstrap::BootstrapInitiatorExt;
use rsnano_rpc_messages::{BootstrapLazyArgs, BootstrapLazyDto, ErrorDto, RpcDto};

impl RpcCommandHandler {
    pub(crate) fn bootstrap_lazy(&self, args: BootstrapLazyArgs) -> RpcDto {
        if self.node.flags.disable_lazy_bootstrap {
            return RpcDto::Error(ErrorDto::LazyBootstrapDisabled);
        }

        let force = args.force.unwrap_or(false);
        let existed = self.node.bootstrap_initiator.current_lazy_attempt();
        let bootstrap_id = args.id.unwrap_or_default();

        let key_inserted =
            self.node
                .bootstrap_initiator
                .bootstrap_lazy(args.hash.into(), force, bootstrap_id);

        let started = !existed.is_some() && key_inserted;

        RpcDto::BootstrapLazy(BootstrapLazyDto::new(started, key_inserted))
    }
}
