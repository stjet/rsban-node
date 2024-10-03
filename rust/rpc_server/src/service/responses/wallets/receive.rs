use std::sync::Arc;
use rsnano_core::PendingKey;
use rsnano_node::{node::Node, wallets::WalletsExt};
use rsnano_rpc_messages::{BlockHashRpcMessage, ErrorDto, ReceiveArgs};
use serde_json::to_string_pretty;

pub async fn receive(node: Arc<Node>, enable_control: bool, args: ReceiveArgs) -> String {
    if !enable_control {
        return to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap();
    }

    let txn = node.ledger.read_txn();
    
    // Check if the block exists
    if !node.ledger.any().block_exists(&txn, &args.block) {
        return to_string_pretty(&ErrorDto::new("Block not found".to_string())).unwrap();
    }

    // Check if the block is pending for the account
    let pending_info = node.ledger.any().get_pending(&txn, &PendingKey::new(args.account, args.block));
    if pending_info.is_none() {
        return to_string_pretty(&ErrorDto::new("Block is not receivable".to_string())).unwrap();
    }

    // Get representative for new accounts
    let representative = node.wallets.get_representative(args.wallet).unwrap_or_default();

    let wallets = node.wallets.mutex.lock().unwrap();
    let wallet = wallets.get(&args.wallet).unwrap().to_owned();

    let block = node.ledger.any().get_block(&node.ledger.read_txn(), &args.block).unwrap();

    // Perform receive action
    let receive = node.wallets.receive_sync(wallet, &block, representative, node.config.receive_minimum);

    match receive {
        Ok(_) => to_string_pretty(&BlockHashRpcMessage::new("block".to_string(), block.hash())).unwrap(),
        Err(_) => to_string_pretty(&ErrorDto::new("Receive error".to_string())).unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use rsnano_core::{Amount, BlockHash, WalletId, DEV_GENESIS_KEY};
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH};
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::{assert_timely_msg, System};
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;

    #[test]
    fn receive() {
        let mut system = System::new();
        let node = system.make_node();

        let wallet = WalletId::zero();
        node.wallets.create(wallet);
        node.wallets.insert_adhoc2(&wallet, &DEV_GENESIS_KEY.private_key(), false).unwrap();

        let key1 = rsnano_core::KeyPair::new();
        node.wallets.insert_adhoc2(&wallet, &key1.private_key(), false).unwrap();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        // Send first block (above minimum receive amount)
        let send1 = node.wallets.send_action2(
            &wallet,
            *DEV_GENESIS_ACCOUNT,
            key1.public_key().into(),
            node.config.receive_minimum,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
            true,
            None
        ).unwrap();

        assert_timely_msg(
            Duration::from_secs(5),
            || node.ledger.any().account_balance(&node.ledger.read_txn(), &(*DEV_GENESIS_ACCOUNT)) != Some(Amount::MAX),
            "Genesis account balance not updated",
        );

        assert_timely_msg(
            Duration::from_secs(10),
            || !node.ledger.any().get_account(&node.ledger.read_txn(), &key1.public_key().into()).is_some(),
            "Destination account should not exist yet",
        );

        // Send second block (below minimum receive amount)
        let send2 = node.wallets.send_action2(
            &wallet,
            *DEV_GENESIS_ACCOUNT,
            key1.public_key().into(),
            node.config.receive_minimum - Amount::raw(1),
            node.work_generate_dev(send1.hash().into()),
            true,
            None
        ).unwrap();

        // Receive the second block
        let block_hash = node.tokio.block_on(async {
            rpc_client
                .receive(
                    wallet,
                    key1.public_key().into(),
                    send2.hash(),
                    None,
                )
                .await
                .unwrap()
        }).value;

        // Verify that the receive transaction was processed
        let tx = node.ledger.read_txn();
        assert_timely_msg(
            Duration::from_secs(5),
            || {
                node.ledger
                    .get_block(&tx, &block_hash)
                    .is_some()
            },
            "Receive block not found in ledger",
        );

        // Verify the balance of the receiving account
        assert_eq!(
            node.ledger.any().account_balance(&tx, &key1.public_key().into()).unwrap(),
            node.config.receive_minimum - Amount::raw(1)
        );

        // Try to receive the same block again (should fail)
        let error_result = node.tokio.block_on(async {
            rpc_client
                .receive(
                    wallet,
                    key1.public_key().into(),
                    send2.hash(),
                    None,
                )
                .await
        });

        assert_eq!(
            error_result.err().map(|e| e.to_string()),
            Some("node returned error: \"Block is not receivable\"".to_string())
        );

        // Try to receive a non-existing block (should fail)
        let non_existing_hash = BlockHash::zero();
        let error_result = node.tokio.block_on(async {
            rpc_client
                .receive(
                    wallet,
                    key1.public_key().into(),
                    non_existing_hash,
                    None,
                )
                .await
        });

        assert_eq!(
            error_result.err().map(|e| e.to_string()),
            Some("node returned error: \"Block not found\"".to_string())
        );

        server.abort();
    }
}