use rsnano_core::Account;
use rsnano_node::{bootstrap::BootstrapInitiatorExt, Node};
use rsnano_rpc_messages::{ErrorDto, RpcDto, SuccessDto};
use std::sync::Arc;

pub async fn bootstrap_any(
    node: Arc<Node>,
    force: Option<bool>,
    id: Option<String>,
    account: Option<Account>,
) -> RpcDto {
    if node.flags.disable_legacy_bootstrap {
        return RpcDto::Error(ErrorDto::LegacyBootstrapDisabled)
    }

    let force = force.unwrap_or(false);
    let bootstrap_id = id.unwrap_or_default();
    let start_account = account.unwrap_or_default();

    node.bootstrap_initiator
        .bootstrap(force, bootstrap_id, u32::MAX, start_account);

    RpcDto::BootstrapAny(SuccessDto::new())
}
