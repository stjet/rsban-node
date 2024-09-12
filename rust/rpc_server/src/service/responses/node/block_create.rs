use std::sync::Arc;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{BlockCreateArgs, BlockCreateDto, ErrorDto};
use rsnano_core::{Account, BlockBuilder, BlockType, KeyPair, PublicKey, RawKey, WorkVersion};
use serde_json::to_string_pretty;

pub async fn block_create(node: Arc<Node>, enable_control: bool, args: BlockCreateArgs) -> String {
    let work_version = args.version.unwrap_or(WorkVersion::Work1);
    let difficulty = args.difficulty.unwrap(); //_or_else(|| node.work_thresholds.threshold_base(work_version));

    let wallet = args.wallet;
    let account = args.account;
    let representative = args.representative;
    let destination = args.destination;
    let source = args.source;
    let amount = args.balance;
    let work = args.work;

    let mut previous = args.previous;
    let mut balance = args.balance;
    let mut prv_key = RawKey::default();

    if work.is_none() && !node.distributed_work.work_generation_enabled() {
        return to_string_pretty(&ErrorDto::new("Work generation is disabled".to_string())).unwrap()
    }

    if let (Some(wallet_id), Some(account)) = (wallet, account) {
        if let Err(e) = node.wallets.fetch(&wallet_id, &account.into()) {
            return to_string_pretty(&ErrorDto::new(e.to_string())).unwrap();
        }
        let tx = node.ledger.read_txn();
        previous = node.ledger.any().account_head(&tx, &account.into()).unwrap();
        balance = node.ledger.any().account_balance(&tx, &account.into()).unwrap();
    }

    if let Some(key) = args.key {
        prv_key = key;
    }

    if prv_key.is_zero() {
        return to_string_pretty(&ErrorDto::new("Block create key required".to_string())).unwrap();
    }

    let pub_key: PublicKey = (&prv_key).try_into().unwrap();
    let pub_key: Account = pub_key.into();

    // Validate account if provided
    if let Some(account) = account {
        if account != pub_key {
            return to_string_pretty(&ErrorDto::new("Block create public key mismatch".to_string())).unwrap();
        }
    }

    let key_pair: KeyPair = prv_key.into();

    let block = match args.block_type {
        BlockType::State => {
            if !representative.is_zero() && (!args.link.unwrap().is_zero() || args.link.is_some()) {
                let builder = BlockBuilder::state();
                builder.account(pub_key)
                    .previous(previous)
                    .representative(representative)
                    .balance(balance)
                    .link(args.link.unwrap_or_default())
                    .sign(&key_pair)
                    .build()
            } else {
                return to_string_pretty(&ErrorDto::new("Invalid block type".to_string())).unwrap();
            }
        },
        BlockType::LegacyOpen => {
            if !representative.is_zero() && source.is_some() {
                let builder = BlockBuilder::legacy_open();
                builder.account(pub_key)
                    .source(source.unwrap())
                    .representative(representative.into())
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
            if !representative.is_zero() {
                let builder = BlockBuilder::legacy_change();
                builder.previous(previous)
                    .representative(representative.into())
                    .sign(&key_pair)
                    .build()
            } else {
                return to_string_pretty(&ErrorDto::new("Invalid block type".to_string())).unwrap();
            }
        },
        BlockType::LegacySend => {
            if destination.is_some() && !balance.is_zero() && !amount.is_zero() {
                if balance >= amount {
                    let mut builder = BlockBuilder::legacy_send();
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

    /*if work.is_none() {
        let difficulty = if args.difficulty.is_none() {
            node.ledger.difficulty(&block)
        } else {
            difficulty
        };

        let work = node.work_generate(work_version, root, difficulty, Some(pub_key)).await?;
        block.set_work(work);
    } else {
        block.set_work(work.unwrap());
    }*/

    let hash = block.hash();
    //let difficulty = node.work_difficulty(&block);
    let json_block = block.json_representation();

    to_string_pretty(&BlockCreateDto::new(hash, 0, json_block)).unwrap()
}