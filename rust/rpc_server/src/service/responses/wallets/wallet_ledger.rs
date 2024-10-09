use rsnano_core::Account;
use rsnano_node::Node;
use rsnano_rpc_messages::{AccountInfo, ErrorDto, WalletLedgerArgs, WalletLedgerDto};
use serde_json::to_string_pretty;
use std::collections::HashMap;
use std::sync::Arc;

pub async fn wallet_ledger(
    node: Arc<Node>,
    enable_control: bool,
    args: WalletLedgerArgs,
) -> String {
    if !enable_control {
        return to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap();
    }

    let WalletLedgerArgs {
        wallet,
        representative,
        weight,
        receivable,
        modified_since,
    } = args;

    let representative = representative.unwrap_or(false);
    let weight = weight.unwrap_or(false);
    let receivable = receivable.unwrap_or(false);
    let modified_since = modified_since.unwrap_or(0);

    match node.wallets.get_accounts_of_wallet(&wallet) {
        Ok(accounts) => {
            let accounts_json = get_accounts_info(
                node,
                accounts,
                representative,
                weight,
                receivable,
                modified_since,
            );
            to_string_pretty(&WalletLedgerDto {
                accounts: accounts_json,
            })
            .unwrap()
        }
        Err(e) => to_string_pretty(&ErrorDto::new(e.to_string())).unwrap(),
    }
}

fn get_accounts_info(
    node: Arc<Node>,
    accounts: Vec<Account>,
    representative: bool,
    weight: bool,
    receivable: bool,
    modified_since: u64,
) -> HashMap<Account, AccountInfo> {
    let block_transaction = node.store.tx_begin_read();
    let mut accounts_json = HashMap::new();

    for account in accounts {
        if let Some(info) = node.ledger.any().get_account(&block_transaction, &account) {
            if info.modified >= modified_since {
                let entry = AccountInfo::new(
                    info.head,
                    info.open_block,
                    node.ledger
                        .representative_block_hash(&block_transaction, &info.head),
                    info.balance,
                    info.modified,
                    info.block_count,
                    representative.then(|| info.representative.as_account()),
                    weight.then(|| node.ledger.weight_exact(&block_transaction, account.into())),
                    receivable.then(|| {
                        node.ledger
                            .account_receivable(&block_transaction, &account, false)
                    }),
                    receivable.then(|| {
                        node.ledger
                            .account_receivable(&block_transaction, &account, false)
                    }),
                );

                accounts_json.insert(account, entry);
            }
        }
    }

    accounts_json
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::{
        Amount, BlockEnum, BlockHash, KeyPair, StateBlock, WalletId, DEV_GENESIS_KEY,
    };
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::{setup_rpc_client_and_server, System};

    fn setup_test_environment(node: Arc<Node>, keys: KeyPair, send_amount: Amount) -> BlockHash {
        let send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            Amount::MAX - send_amount,
            keys.account().into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));

        node.process(send1.clone()).unwrap();

        let open_block = BlockEnum::State(StateBlock::new(
            keys.account(),
            BlockHash::zero(),
            keys.public_key(),
            send_amount,
            send1.hash().into(),
            &keys,
            node.work_generate_dev(keys.public_key().into()),
        ));

        node.process(open_block.clone()).unwrap();

        open_block.hash()
    }

    #[test]
    fn wallet_ledger() {
        let mut system = System::new();
        let node = system.build_node().finish();
        let keys = KeyPair::new();
        let send_amount = Amount::from(100);
        let open_hash = setup_test_environment(node.clone(), keys.clone(), send_amount);

        let wallet_id = WalletId::zero();
        node.wallets.create(wallet_id);
        node.wallets
            .insert_adhoc2(&wallet_id, &keys.private_key(), true)
            .unwrap();

        let wallet = wallet_id;
        let representative = Some(true);
        let weight = Some(true);
        let receivable = Some(true);
        let modified_since = None;

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node.runtime.block_on(async {
            rpc_client
                .wallet_ledger(wallet, representative, weight, receivable, modified_since)
                .await
                .unwrap()
        });

        let accounts = result.accounts;

        assert_eq!(accounts.len(), 1);
        let (account, info) = accounts.iter().next().unwrap();
        assert_eq!(*account, keys.account());
        assert_eq!(info.frontier, BlockHash::from(open_hash));
        assert_eq!(info.open_block, BlockHash::from(open_hash));
        assert_eq!(info.representative_block, BlockHash::from(open_hash));
        assert_eq!(info.balance, send_amount);
        assert!(info.modified_timestamp > 0);
        assert_eq!(info.block_count, 1);
        assert_eq!(info.weight, Some(send_amount));
        assert_eq!(info.pending, Some(Amount::zero()));
        assert_eq!(info.receivable, Some(Amount::zero()));
        assert_eq!(info.representative, Some(keys.account()));

        let result_without_optional = node.runtime.block_on(async {
            rpc_client
                .wallet_ledger(wallet, None, None, None, None)
                .await
                .unwrap()
        });

        let accounts_without_optional = result_without_optional.accounts;
        let (_, info_without_optional) = accounts_without_optional.iter().next().unwrap();
        assert!(info_without_optional.weight.is_none());
        assert!(info_without_optional.pending.is_none());
        assert!(info_without_optional.receivable.is_none());
        assert!(info_without_optional.representative.is_none());

        server.abort();
    }

    #[test]
    fn account_create_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node.runtime.block_on(async {
            rpc_client
                .wallet_ledger(WalletId::zero(), None, None, None, None)
                .await
        });

        assert_eq!(
            result.err().map(|e| e.to_string()),
            Some("node returned error: \"RPC control is disabled\"".to_string())
        );

        server.abort();
    }

    #[test]
    fn account_create_fails_with_wallet_not_found() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node.runtime.block_on(async {
            rpc_client
                .wallet_ledger(WalletId::zero(), None, None, None, None)
                .await
        });

        assert_eq!(
            result.err().map(|e| e.to_string()),
            Some("node returned error: \"Wallet not found\"".to_string())
        );

        server.abort();
    }
}
