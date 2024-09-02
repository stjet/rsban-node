use rsnano_core::{Account, WalletId};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{ErrorDto, SuccessDto};
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn work_set(
    node: Arc<Node>,
    enable_control: bool,
    wallet: WalletId,
    account: Account,
    work: u64,
) -> String {
    match node.wallets.work_set(&wallet, &account.into(), work) {
        Ok(_) => to_string_pretty(&SuccessDto::new()).unwrap(),
        Err(e) => to_string_pretty(&ErrorDto::new(e.to_string())).unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::{Account, WalletId};
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::System;

    #[test]
    fn work_set() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        node.wallets.create(WalletId::zero());

        node.tokio.block_on(async {
            rpc_client
                .work_set(WalletId::zero(), Account::zero(), 1)
                .await
                .unwrap()
        });

        server.abort();
    }

    #[test]
    fn work_set_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node.tokio.block_on(async {
            rpc_client
                .work_set(WalletId::zero(), Account::zero(), 1)
                .await
        });

        assert!(result.is_err());

        server.abort();
    }
}
