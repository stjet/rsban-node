use std::sync::Arc;
use rsnano_core::BlockHash;
use rsnano_node::{node::Node, wallets::WalletsExt};
use rsnano_rpc_messages::{BlockHashRpcMessage, ErrorDto, ReceiveArgs};
use serde_json::to_string_pretty;

pub async fn receive(node: Arc<Node>, enable_control: bool, args: ReceiveArgs) -> String {
    if enable_control {
        let txn = node.ledger.read_txn();
        let wallets = node.wallets.mutex.lock().unwrap();
        let wallet = wallets.get(&args.wallet).unwrap();
        let block = node.ledger.get_block(&txn, &args.block).unwrap();
        //let receive = node.wallets.receive_action(wallet, args.block, block.representative_field().unwrap(), block.balance(), args.account, args.work.unwrap_or(0.into()).into(), !args.work.is_some()).unwrap();
        to_string_pretty(&BlockHashRpcMessage::new("block".to_string(), BlockHash::zero())).unwrap()
    }
    else {
        to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;
    use rsnano_core::{Account, WalletId, DEV_GENESIS_KEY, Amount, BlockEnum, StateBlock};
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
    use rsnano_node::{node::Node, wallets::WalletsExt};
    use test_helpers::{System, assert_timely_msg};
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;

    fn send_block(node: Arc<Node>, account: Account, amount: Amount) -> BlockEnum {
        let send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            Amount::MAX - amount,
            account.into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));

        node.process_active(send1.clone());
        assert_timely_msg(
            Duration::from_secs(5),
            || node.active.active(&send1),
            "not active on node 1",
        );
        send1
    }

    #[test]
    fn receive() {
        let mut system = System::new();
        let node1 = system.make_node();
        let node2 = system.make_node();

        let wallet = WalletId::zero();
        node1.wallets.create(wallet);
        node1.wallets.insert_adhoc2(&wallet, &DEV_GENESIS_KEY.private_key(), false).unwrap();

        let (rpc_client, server) = setup_rpc_client_and_server(node1.clone(), true);

        // Send a block first to create a receivable
        let destination = Account::decode_account("nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3").unwrap();
        let amount = Amount::raw(1000000);

        let send_result = send_block(node1.clone(), destination, amount);

        // Now test the receive function
        let receive_result = node1.tokio.block_on(async {
            rpc_client
                .receive(
                        wallet,
                        destination,
                        send_result.hash(),
                        None,
                    
                )
                .await
                .unwrap()
        });

        let tx = node1.ledger.read_txn();

        // Verify that the receive transaction was processed
        assert_timely_msg(
            Duration::from_secs(5),
            || {
                node1.ledger
                    .get_block(&tx, &receive_result.value)
                    .is_some()
            },
            "Receive block not found in ledger",
        );

        // Verify the balance of the receiving account
        assert_eq!(
            node1.ledger.any().account_balance(&tx, &destination).unwrap(),
            amount
        );

        server.abort();
    }
}