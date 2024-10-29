use crate::command_handler::RpcCommandHandler;
use anyhow::bail;
use rsnano_node::bootstrap::BootstrapInitiatorExt;
use rsnano_rpc_messages::{BootstrapLazyArgs, BootstrapLazyResponse};

impl RpcCommandHandler {
    pub(crate) fn bootstrap_lazy(
        &self,
        args: BootstrapLazyArgs,
    ) -> anyhow::Result<BootstrapLazyResponse> {
        if self.node.flags.disable_lazy_bootstrap {
            bail!("Lazy bootstrap is disabled");
        }

        let force = args.force.unwrap_or(false);
        let existed = self
            .node
            .bootstrap_initiator
            .current_lazy_attempt()
            .is_some();
        let bootstrap_id = args.id.unwrap_or_default();

        let key_inserted =
            self.node
                .bootstrap_initiator
                .bootstrap_lazy(args.hash.into(), force, bootstrap_id);

        let started = !existed && key_inserted;
        Ok(BootstrapLazyResponse {
            started: if started { 1 } else { 0 },
            key_inserted: if key_inserted { 1 } else { 0 },
        })
    }
}
