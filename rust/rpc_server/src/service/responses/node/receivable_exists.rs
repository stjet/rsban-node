use rsnano_core::BlockHash;
use rsnano_node::Node;
use rsnano_rpc_messages::BoolDto;
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn receivable_exists(
    node: Arc<Node>,
    hash: BlockHash,
    include_active: Option<bool>,
    include_only_confirmed: Option<bool>,
) -> String {
    let include_active = include_active.unwrap_or(false);
    let include_only_confirmed = include_only_confirmed.unwrap_or(true);
    let txn = node.ledger.read_txn();

    let exists = if let Some(block) = node.ledger.get_block(&txn, &hash) {
        if block.is_send() {
            let pending_key = rsnano_core::PendingKey::new(block.destination().unwrap(), hash);
            let pending_exists = node.ledger.any().get_pending(&txn, &pending_key).is_some();

            if pending_exists {
                block_confirmed(node.clone(), &hash, include_active, include_only_confirmed)
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    to_string_pretty(&BoolDto::new("exists".to_string(), exists)).unwrap()
}

fn block_confirmed(
    node: Arc<Node>,
    hash: &BlockHash,
    include_active: bool,
    include_only_confirmed: bool,
) -> bool {
    let txn = node.ledger.read_txn();

    if include_active && !include_only_confirmed {
        return true;
    }

    if node.ledger.confirmed().block_exists_or_pruned(&txn, hash) {
        return true;
    }

    if !include_only_confirmed {
        if let Some(block) = node.ledger.get_block(&txn, hash) {
            return !node.active.active(&block);
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::{Amount, BlockEnum, StateBlock, DEV_GENESIS_KEY};
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
    use rsnano_node::Node;
    use std::sync::Arc;
    use std::time::Duration;
    use test_helpers::{assert_timely_msg, setup_rpc_client_and_server, System};

    fn send_block(node: Arc<Node>) -> BlockEnum {
        let send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            Amount::MAX - Amount::raw(1),
            DEV_GENESIS_KEY.account().into(),
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
    fn receivable_exists_confirmed() {
        let mut system = System::new();
        let node = system.make_node();

        let send = send_block(node.clone());
        node.confirm(send.hash().clone());

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node.runtime.block_on(async {
            rpc_client
                .receivable_exists(send.hash(), None, Some(true))
                .await
                .unwrap()
        });

        assert_eq!(result.value, true);

        server.abort();
    }

    #[test]
    fn test_receivable_exists_unconfirmed() {
        let mut system = System::new();
        let node = system.make_node();

        let send = send_block(node.clone());

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node.runtime.block_on(async {
            rpc_client
                .receivable_exists(send.hash(), Some(true), Some(false))
                .await
                .unwrap()
        });

        assert_eq!(result.value, true);

        server.abort();
    }

    #[test]
    fn test_receivable_exists_non_existent() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let non_existent_hash = BlockHash::zero();
        let result = node.runtime.block_on(async {
            rpc_client
                .receivable_exists(non_existent_hash, None, None)
                .await
                .unwrap()
        });

        assert_eq!(result.value, false);

        server.abort();
    }
}
