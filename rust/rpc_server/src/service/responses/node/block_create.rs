use std::sync::Arc;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{BlockCreateArgs, BlockCreateDto, ErrorDto};
use rsnano_core::{Account, Amount, BlockBuilder, BlockDetails, BlockEnum, BlockHash, BlockType, Epoch, KeyPair, PendingKey, PublicKey, RawKey, WorkVersion};
use serde_json::to_string_pretty;

pub async fn block_create(node: Arc<Node>, enable_control: bool, args: BlockCreateArgs) -> String {
    if !enable_control {
        return to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap();
    }

    let work_version = args.version.unwrap_or(WorkVersion::Work1);
    let difficulty = args.difficulty.unwrap_or_else(|| node.ledger.constants.work.threshold_base(work_version));

    let wallet = args.wallet;
    let account = args.account;
    let representative = args.representative;
    let destination = args.destination;
    let source = args.source;
    let amount = args.balance;
    let work = args.work;

    let mut previous = args.previous.unwrap_or(BlockHash::zero());
    let mut balance = args.balance.unwrap_or(Amount::zero());
    let mut prv_key = RawKey::default();

    if work.is_none() && !node.distributed_work.work_generation_enabled() {
        return to_string_pretty(&ErrorDto::new("Work generation is disabled".to_string())).unwrap()
    }

    if let (Some(wallet_id), Some(account)) = (wallet, account) {
        if let Err(e) = node.wallets.fetch(&wallet_id, &account.into()) {
            return to_string_pretty(&ErrorDto::new(e.to_string())).unwrap();
        }
        let tx = node.ledger.read_txn();
        previous = node.ledger.any().account_head(&tx, &account).unwrap();
        balance = node.ledger.any().account_balance(&tx, &account).unwrap();
    }

    if let Some(key) = args.key {
        prv_key = key;
    }

    if prv_key.is_zero() {
        return to_string_pretty(&ErrorDto::new("Block create key required".to_string())).unwrap();
    }

    let pub_key: PublicKey = (&prv_key).try_into().unwrap();
    let pub_key: Account = pub_key.into();

    if let Some(account) = account {
        if account != pub_key {
            return to_string_pretty(&ErrorDto::new("Block create public key mismatch".to_string())).unwrap();
        }
    }

    let key_pair: KeyPair = prv_key.into();

    let mut block = match args.block_type {
        BlockType::State => {
            if !representative.is_none() && (!args.link.unwrap_or_default().is_zero() || args.link.is_some()) {
                let builder = BlockBuilder::state();
                builder.account(pub_key)
                    .previous(previous)
                    .representative(representative.unwrap())
                    .balance(balance)
                    .link(args.link.unwrap_or_default())
                    .sign(&key_pair)
                    .build()
            } else {
                return to_string_pretty(&ErrorDto::new("Invalid block type".to_string())).unwrap();
            }
        },
        BlockType::LegacyOpen => {
            if !representative.is_none() && source.is_some() {
                let builder = BlockBuilder::legacy_open();
                builder.account(pub_key)
                    .source(source.unwrap())
                    .representative(representative.unwrap().into())
                    .sign(&key_pair)
                    .build()
            } else {
                return to_string_pretty(&ErrorDto::new("Invalid block type".to_string())).unwrap();
            }
        },
        BlockType::LegacyReceive => {
            if source.is_some() {
                let builder = BlockBuilder::legacy_receive();
                builder.previous(previous)
                    .source(source.unwrap())
                    .sign(&key_pair)
                    .build()
            } else {
                return to_string_pretty(&ErrorDto::new("Invalid block type".to_string())).unwrap();
            }
        },
        BlockType::LegacyChange => {
            if !representative.is_none() {
                let builder = BlockBuilder::legacy_change();
                builder.previous(previous)
                    .representative(representative.unwrap().into())
                    .sign(&key_pair)
                    .build()
            } else {
                return to_string_pretty(&ErrorDto::new("Invalid block type".to_string())).unwrap();
            }
        },
        BlockType::LegacySend => {
            if destination.is_some() && !balance.is_zero() && !amount.is_none() {
                let amount = amount.unwrap();
                if balance >= amount {
                    let builder = BlockBuilder::legacy_send();
                    builder.previous(previous)
                        .destination(destination.unwrap())
                        .balance(balance - amount)
                        .sign(key_pair)
                        .build()
                } else {
                    return to_string_pretty(&ErrorDto::new("Insufficient balance".to_string())).unwrap();
                }
            } else {
                return to_string_pretty(&ErrorDto::new("Invalid block type".to_string())).unwrap();
            }
        }
        _ => return to_string_pretty(&ErrorDto::new("Invalid block type".to_string())).unwrap(),
    };

    let root = if !previous.is_zero() { previous } else { pub_key.into() };

    if work.is_none() {
        let difficulty = if args.difficulty.is_none() {
            difficulty_ledger(node.clone(),&block)
        } else {
            difficulty
        };

        let work = match node.distributed_work.make(root.into(), difficulty, Some(pub_key)).await {
            Some(work) => work,
            None => return to_string_pretty(&ErrorDto::new("Failed to generate work".to_string())).unwrap(),
        };
        block.set_work(work);
    } else {
        block.set_work(work.unwrap().into());
    }

    let hash = block.hash();
    let difficulty = block.work();
    let json_block = block.json_representation();

    to_string_pretty(&BlockCreateDto::new(hash, difficulty, json_block)).unwrap()
}

pub fn difficulty_ledger(node: Arc<Node>, block: &BlockEnum) -> u64 {
    let mut details = BlockDetails::new(Epoch::Epoch0, false, false, false);
    let mut details_found = false;

    let transaction = node.store.tx_begin_read();
    
    // Previous block find
    let mut block_previous: Option<BlockEnum> = None;
    let previous = block.previous();
    if !previous.is_zero() {
        block_previous = node.ledger.any().get_block(&transaction, &previous);
    }

    // Send check
    if let Some(prev_block) = &block_previous {
        let is_send = node.ledger.any().block_balance(&transaction, &previous) > block.balance_field();
        details = BlockDetails::new(Epoch::Epoch0, is_send, false, false);
        details_found = true;
    }

    // Epoch check
    if let Some(prev_block) = &block_previous {
        let epoch = prev_block.sideband().unwrap().details.epoch;
        details = BlockDetails::new(epoch, details.is_send, details.is_receive, details.is_epoch);
    }

    // Link check
    if let Some(link) = block.link_field() {
        if !details.is_send {
            if let Some(block_link) = node.ledger.any().get_block(&transaction, &link.into()) {
                let account = block.account_field().unwrap();
                if node.ledger.any().get_pending(&transaction, &PendingKey::new(account, link.into())).is_some() {
                    let epoch = std::cmp::max(details.epoch, block_link.sideband().unwrap().details.epoch);
                    details = BlockDetails::new(epoch, details.is_send, true, details.is_epoch);
                    details_found = true;
                }
            }
        }
    }

    if details_found {
        node.network_params.work.threshold(&details)
    } else {
        node.network_params.work.threshold_base(block.work_version())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::{Amount, StateBlock, WalletId, DEV_GENESIS_KEY};
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::System;
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;

    #[test]
    fn block_create_state() {
        let mut system = System::new();
        let mut config = System::default_config();
        config.online_weight_minimum = Amount::MAX;
        let node = system.build_node().config(config).finish();

        let wallet_id = WalletId::zero();
        node.wallets.create(wallet_id);
        node.wallets.insert_adhoc2(&wallet_id, &DEV_GENESIS_KEY.private_key(), false).unwrap();

        let (rpc_client, _server) = setup_rpc_client_and_server(node.clone(), true);

        // Create and process send1 block
        let key1 = KeyPair::new();
        let send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            Amount::MAX - Amount::raw(100),
            key1.account().into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));

        println!("Send1 block: {:?}", &send1);
        node.process(send1.clone()).unwrap();

        // Create receive block for key1
        let result = node.tokio.block_on(async {
            rpc_client
                .block_create(
                    BlockType::State,
                    Some(Amount::raw(100)),
                    Some(key1.private_key()),
                    None,
                    Some(key1.account()),
                    None,
                    None,
                    Some(key1.account()),
                    Some(send1.hash().into()),
                    None,
                    None,
                    None,
                    None,
                )
                .await
                .unwrap()
        });

        let block_hash = result.hash;
        let block: BlockEnum = result.block.into();

        assert_eq!(block.block_type(), BlockType::State);
        assert_eq!(block.hash(), block_hash);

        println!("Receive block: {:?}", block);

        // Process the receive block
        node.process(block.clone()).unwrap();

        // Verify the balance of key1's account
        let tx = node.ledger.read_txn();
        let balance = node.ledger.any().account_balance(&tx, &key1.account()).unwrap();
        assert_eq!(balance, Amount::raw(100));
    }
}