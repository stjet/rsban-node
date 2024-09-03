use rsnano_core::{Account, WalletId};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{ErrorDto, SuccessDto, U64RpcMessage};
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn work_get(
    node: Arc<Node>,
    enable_control: bool,
    wallet: WalletId,
    account: Account,
) -> String {
    if enable_control {
        match node.wallets.work_get2(&wallet, &account.into()) {
            Ok(work) => to_string_pretty(&U64RpcMessage::new("work".to_string(), work)).unwrap(),
            Err(e) => to_string_pretty(&ErrorDto::new(e.to_string())).unwrap(),
        }
    } else {
        to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::{Account, WalletId};
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::System;

    #[test]
    fn work_get() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet = WalletId::zero();
        let account = Account::zero();

        node.wallets.create(wallet);

        node.wallets.work_set(&wallet, &account.into(), 1).unwrap();

        let result = node
            .tokio
            .block_on(async { rpc_client.work_get(wallet, account).await.unwrap() });

        assert_eq!(result.value, 1);

        server.abort();
    }

    #[test]
    fn work_get_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node
            .tokio
            .block_on(async { rpc_client.work_get(WalletId::zero(), Account::zero()).await });

        assert_eq!(
            result.err().map(|e| e.to_string()),
            Some("node returned error: \"RPC control is disabled\"".to_string())
        );

        server.abort();
    }
}
