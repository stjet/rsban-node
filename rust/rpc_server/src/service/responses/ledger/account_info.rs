use rsnano_core::Account;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{AccountInfoArgs, AccountInfoDto};
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn account_info(
    node: Arc<Node>,
    args: AccountInfoArgs,
) -> String {
    let txn = node.ledger.read_txn();
    let include_confirmed = args.include_confirmed.unwrap_or(false);

    let info = node.ledger.any().get_account(&txn, &args.account).unwrap();

    let confirmation_height_info = node.store.confirmation_height.get(&txn, &args.account).unwrap();

    let mut account_info = AccountInfoDto::new(
        info.head,
        info.open_block,
        node.ledger.representative_block_hash(&txn, &info.head),
        info.balance,
        info.modified,
        info.block_count,
        info.epoch as u8,
    );

    account_info.confirmed_height = Some(confirmation_height_info.height);
    account_info.confirmation_height_frontier = Some(confirmation_height_info.frontier);

    to_string_pretty(&account_info).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_ledger::DEV_GENESIS_ACCOUNT;
    use test_helpers::System;

    #[test]
    fn account_info() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node
            .tokio
            .block_on(async { rpc_client.account_info(*DEV_GENESIS_ACCOUNT, None, None, None, None, None).await.unwrap() });

        server.abort();
    }
}