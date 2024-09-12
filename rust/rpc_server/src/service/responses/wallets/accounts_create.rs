use rsnano_core::Account;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{AccountsCreateArgs, AccountsRpcMessage, ErrorDto};
use serde_json::to_string_pretty;
use crate::RpcService;

pub async fn accounts_create(rpc_service: RpcService, args: AccountsCreateArgs) -> String {
    if rpc_service.enable_control {
        let work = args.work.unwrap_or(false);
        let mut accounts: Vec<Account> = vec![];
        for _ in 0..args.wallet_with_count.count as usize {
            match rpc_service.node.wallets.deterministic_insert2(&args.wallet_with_count.wallet, work) {
                Ok(account) => accounts.push(account.into()),
                Err(e) => return to_string_pretty(&ErrorDto::new(e.to_string())).unwrap(),
            }
        }
        to_string_pretty(&AccountsRpcMessage::new(accounts)).unwrap()
    } else {
        to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::WalletId;
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::System;

    #[test]
    fn accounts_create() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet = WalletId::random();

        node.wallets.create(wallet);

        node.tokio
            .block_on(async { rpc_client.accounts_create(wallet, 8, Some(true)).await.unwrap() });

        assert_eq!(
            node.wallets.get_accounts_of_wallet(&wallet).unwrap().len(),
            8
        );

        server.abort();
    }

    #[test]
    fn accounts_create_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let wallet = WalletId::random();

        node.wallets.create(wallet);

        let result = node
            .tokio
            .block_on(async { rpc_client.accounts_create(wallet, 8, None).await });

        assert!(result.is_err());

        server.abort();
    }
}
