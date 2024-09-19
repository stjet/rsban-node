use std::sync::Arc;
use rsnano_core::{BlockEnum, BlockHash, PendingKey, StateBlock, WorkVersion};
use rsnano_node::{node::Node, wallets::WalletsExt, work::WorkRequest};
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

    // Perform receive action
    let receive = node.wallets.receive_action2(
        &args.wallet,
        args.block,
        representative,
        pending_info.unwrap().amount,
        args.account,
        args.work.unwrap_or(0.into()).into(),
        args.work.is_none()
    );

    match receive {
        Ok(Some(block)) => to_string_pretty(&BlockHashRpcMessage::new("block".to_string(), block.hash())).unwrap(),
        Ok(None) => to_string_pretty(&ErrorDto::new("Failed to create receive block".to_string())).unwrap(),
        Err(e) => to_string_pretty(&ErrorDto::new(e.to_string())).unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use rsnano_core::{Amount, BlockHash, WalletId, DEV_GENESIS_KEY};
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH};
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::{assert_timely_msg, establish_tcp, System};
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;

    #[test]
    fn receive() {
        let mut system = System::new();
        let node1 = system.make_node();
        let node2 = system.make_node();
        establish_tcp(&node1, &node2);

        let wallet = WalletId::zero();
        node1.wallets.create(wallet);
        node1.wallets.insert_adhoc2(&wallet, &DEV_GENESIS_KEY.private_key(), false).unwrap();

        let wallet2 = WalletId::random();
        node2.wallets.create(wallet2);
        let key1 = rsnano_core::KeyPair::new();
        node2.wallets.insert_adhoc2(&wallet2, &key1.private_key(), false).unwrap();

        let (rpc_client, server) = setup_rpc_client_and_server(node2.clone(), true);

        // Send first block (above minimum receive amount)
        let send1 = node1.wallets.send_action2(
            &wallet,
            *DEV_GENESIS_ACCOUNT,
            key1.public_key().into(),
            node1.config.receive_minimum,
            node1.work_generate_dev((*DEV_GENESIS_HASH).into()),
            true,
            None
        ).unwrap();

        assert_timely_msg(
            Duration::from_secs(5),
            || node1.ledger.any().account_balance(&node1.ledger.read_txn(), &(*DEV_GENESIS_ACCOUNT)) != Some(Amount::MAX),
            "Genesis account balance not updated",
        );

        assert_timely_msg(
            Duration::from_secs(10),
            || !node1.ledger.any().get_account(&node1.ledger.read_txn(), &key1.public_key().into()).is_some(),
            "Destination account should not exist yet",
        );

        // Send second block (below minimum receive amount)
        /*let send2 = node.wallets.send_action2(
            &wallet,
            *DEV_GENESIS_ACCOUNT,
            key1.public_key().into(),
            node.config.receive_minimum - Amount::raw(1),
            node.work_generate_dev(send1.hash().into()),
            true,
            None
        ).unwrap();*/

        // Receive the second block
        let block_hash = node2.tokio.block_on(async move {
            rpc_client
                .receive(
                    wallet2,
                    key1.public_key().into(),
                    send1.hash(),
                    None,
                )
                .await
                .unwrap()
        }).value;

        println!("{:?}", block_hash);

        // Verify that the receive transaction was processed
        /*let tx = node2.ledger.read_txn();
        assert_timely_msg(
            Duration::from_secs(5),
            || {
                node2.ledger
                    .get_block(&tx, &receive_result)
                    .is_some()
            },
            "Receive block not found in ledger",
        );

        // Verify the balance of the receiving account
        assert_eq!(
            node2.ledger.any().account_balance(&tx, &key1.public_key().into()).unwrap(),
            node2.config.receive_minimum - Amount::raw(1)
        );

        // Try to receive the same block again (should fail)
        let error_result = node2.tokio.block_on(async {
            rpc_client
                .receive(
                    wallet,
                    key1.public_key().into(),
                    send1.hash(),
                    None,
                )
                .await
        });

        assert!(error_result.is_err());
        assert_eq!(error_result.unwrap_err().to_string(), "Block is not receivable");

        // Try to receive a non-existing block (should fail)
        let non_existing_hash = BlockHash::zero();
        let error_result = node2.tokio.block_on(async {
            rpc_client
                .receive(
                    wallet,
                    key1.public_key().into(),
                    non_existing_hash,
                    None,
                )
                .await
        });

        assert!(error_result.is_err());
        assert_eq!(error_result.unwrap_err().to_string(), "Block not found");*/

        server.abort();
    }
}