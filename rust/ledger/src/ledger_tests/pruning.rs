use std::sync::atomic::Ordering;

use crate::{ledger_constants::LEDGER_CONSTANTS_STUB, ProcessResult, DEV_GENESIS_HASH};
use rsnano_core::{
    work::{WorkPool, STUB_WORK_POOL},
    Amount, BlockBuilder, BlockDetails, Epoch, PendingKey,
};

use crate::ledger_tests::LedgerContext;

use super::upgrade_genesis_to_epoch_v1;

#[test]
fn pruning_action() {
    let ctx = LedgerContext::empty();
    ctx.ledger.enable_pruning();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut send1 = genesis
        .send(txn.txn())
        .amount(100)
        .link(genesis.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let mut send2 = genesis
        .send(txn.txn())
        .amount(100)
        .link(genesis.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    // Prune...
    assert_eq!(ctx.ledger.pruning_action(txn.as_mut(), &send1.hash(), 1), 1);
    assert_eq!(
        ctx.ledger
            .pruning_action(txn.as_mut(), &DEV_GENESIS_HASH, 1),
        0
    );
    assert!(ctx
        .ledger
        .store
        .pending()
        .exists(txn.txn(), &PendingKey::new(genesis.account(), send1.hash())),);

    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &send1.hash()),
        false
    );

    assert!(ctx
        .ledger
        .block_or_pruned_exists_txn(txn.txn(), &send1.hash()),);

    assert!(ctx.ledger.store.pruned().exists(txn.txn(), &send1.hash()),);

    assert!(ctx
        .ledger
        .store
        .block()
        .exists(txn.txn(), &DEV_GENESIS_HASH));
    assert!(ctx.ledger.store.block().exists(txn.txn(), &send2.hash()));

    // Receiving pruned block
    let mut receive1 = BlockBuilder::state()
        .account(genesis.account())
        .previous(send2.hash())
        .balance(LEDGER_CONSTANTS_STUB.genesis_amount - Amount::raw(100))
        .link(send1.hash())
        .sign(&genesis.key)
        .work(STUB_WORK_POOL.generate_dev2(send2.hash().into()).unwrap())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut receive1).unwrap();

    assert!(ctx.ledger.store.block().exists(txn.txn(), &receive1.hash()));
    assert_eq!(
        ctx.ledger
            .store
            .pending()
            .exists(txn.txn(), &PendingKey::new(genesis.account(), send1.hash())),
        false
    );
    let receive1_stored = ctx.ledger.get_block(txn.txn(), &receive1.hash()).unwrap();
    assert_eq!(receive1, receive1_stored);
    assert_eq!(receive1_stored.sideband().unwrap().height, 4);
    assert_eq!(
        receive1_stored.sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch0, false, true, false)
    );

    // Middle block pruning
    assert!(ctx.ledger.store.block().exists(txn.txn(), &send2.hash()));
    assert_eq!(ctx.ledger.pruning_action(txn.as_mut(), &send2.hash(), 1), 1);
    assert!(ctx.ledger.store.pruned().exists(txn.txn(), &send2.hash()));
    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &send2.hash()),
        false
    );
    assert_eq!(
        ctx.ledger.store.account().count(txn.txn()),
        ctx.ledger.cache.account_count.load(Ordering::Relaxed)
    );
    assert_eq!(
        ctx.ledger.store.pruned().count(txn.txn()),
        ctx.ledger.cache.pruned_count.load(Ordering::Relaxed)
    );
    assert_eq!(
        ctx.ledger.store.block().count(txn.txn()),
        ctx.ledger.cache.block_count.load(Ordering::Relaxed)
            - ctx.ledger.cache.pruned_count.load(Ordering::Relaxed)
    );
}
#[test]
fn pruning_large_chain() {
    let ctx = LedgerContext::empty();
    ctx.ledger.enable_pruning();
    let genesis = ctx.genesis_block_factory();
    let mut txn = ctx.ledger.rw_txn();
    let send_receive_pairs = 20;
    let mut last_hash = *DEV_GENESIS_HASH;

    for _ in 0..send_receive_pairs {
        let mut send = genesis.send(txn.txn()).link(genesis.account()).build();
        ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

        let mut receive = genesis.receive(txn.txn(), send.hash()).build();
        ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

        last_hash = receive.hash();
    }
    assert_eq!(
        ctx.ledger.store.block().count(txn.txn()),
        send_receive_pairs * 2 + 1
    );

    // Pruning action
    assert_eq!(
        ctx.ledger.pruning_action(txn.as_mut(), &last_hash, 5),
        send_receive_pairs * 2
    );

    assert!(ctx.ledger.store.pruned().exists(txn.txn(), &last_hash));
    assert!(ctx
        .ledger
        .store
        .block()
        .exists(txn.txn(), &DEV_GENESIS_HASH));
    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &last_hash),
        false
    );
    assert_eq!(
        ctx.ledger.store.pruned().count(txn.txn()),
        ctx.ledger.cache.pruned_count.load(Ordering::Relaxed)
    );
    assert_eq!(
        ctx.ledger.store.block().count(txn.txn()),
        ctx.ledger.cache.block_count.load(Ordering::Relaxed)
            - ctx.ledger.cache.pruned_count.load(Ordering::Relaxed)
    );
    assert_eq!(
        ctx.ledger.store.pruned().count(txn.txn()),
        send_receive_pairs * 2
    );
    assert_eq!(ctx.ledger.store.block().count(txn.txn()), 1);
}

#[test]
fn pruning_source_rollback() {
    let ctx = LedgerContext::empty();
    ctx.ledger.enable_pruning();
    let genesis = ctx.genesis_block_factory();
    let mut txn = ctx.ledger.rw_txn();

    upgrade_genesis_to_epoch_v1(&ctx, txn.as_mut());

    let mut send1 = genesis
        .send(txn.txn())
        .amount(100)
        .link(genesis.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let mut send2 = genesis
        .send(txn.txn())
        .amount(100)
        .link(genesis.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    // Pruning action
    assert_eq!(ctx.ledger.pruning_action(txn.as_mut(), &send1.hash(), 1), 2);

    // Receiving pruned block
    let mut receive1 = BlockBuilder::state()
        .account(genesis.account())
        .previous(send2.hash())
        .balance(LEDGER_CONSTANTS_STUB.genesis_amount - Amount::raw(100))
        .link(send1.hash())
        .sign(&genesis.key)
        .work(STUB_WORK_POOL.generate_dev2(send2.hash().into()).unwrap())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut receive1).unwrap();

    // Rollback receive block
    ctx.ledger.rollback(txn.as_mut(), &receive1.hash()).unwrap();
    let info2 = ctx
        .ledger
        .pending_info(txn.txn(), &PendingKey::new(genesis.account(), send1.hash()))
        .unwrap();
    assert_ne!(info2.source, genesis.account()); // Tradeoff to not store pruned blocks accounts
    assert_eq!(info2.amount, Amount::raw(100));
    assert_eq!(info2.epoch, Epoch::Epoch1);

    // Process receive block again
    ctx.ledger.process(txn.as_mut(), &mut receive1).unwrap();

    assert_eq!(
        ctx.ledger
            .store
            .pending()
            .exists(txn.txn(), &PendingKey::new(genesis.account(), send1.hash())),
        false
    );
    assert_eq!(ctx.ledger.cache.pruned_count.load(Ordering::Relaxed), 2);
    assert_eq!(ctx.ledger.cache.block_count.load(Ordering::Relaxed), 5);
}

#[test]
fn pruning_source_rollback_legacy() {
    let ctx = LedgerContext::empty();
    ctx.ledger.enable_pruning();
    let genesis = ctx.genesis_block_factory();
    let mut txn = ctx.ledger.rw_txn();

    let mut send1 = genesis
        .legacy_send(txn.txn())
        .destination(genesis.account())
        .amount(100)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let destination = ctx.block_factory();
    let mut send2 = genesis
        .legacy_send(txn.txn())
        .destination(destination.account())
        .amount(100)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    let mut send3 = genesis
        .legacy_send(txn.txn())
        .destination(genesis.account())
        .amount(100)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send3).unwrap();

    // Pruning action
    assert_eq!(ctx.ledger.pruning_action(txn.as_mut(), &send2.hash(), 1), 2);

    // Receiving pruned block
    let mut receive1 = BlockBuilder::legacy_receive()
        .previous(send3.hash())
        .source(send1.hash())
        .sign(&genesis.key)
        .work(STUB_WORK_POOL.generate_dev2(send3.hash().into()).unwrap())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut receive1).unwrap();

    // Rollback receive block
    ctx.ledger.rollback(txn.as_mut(), &receive1.hash()).unwrap();

    let info3 = ctx
        .ledger
        .pending_info(txn.txn(), &PendingKey::new(genesis.account(), send1.hash()))
        .unwrap();
    assert_ne!(info3.source, genesis.account()); // Tradeoff to not store pruned blocks accounts
    assert_eq!(info3.amount, Amount::raw(100));
    assert_eq!(info3.epoch, Epoch::Epoch0);

    // Process receive block again
    ctx.ledger.process(txn.as_mut(), &mut receive1).unwrap();

    assert_eq!(
        ctx.ledger
            .pending_info(txn.txn(), &PendingKey::new(genesis.account(), send1.hash())),
        None
    );
    assert_eq!(ctx.ledger.cache.pruned_count.load(Ordering::Relaxed), 2);
    assert_eq!(ctx.ledger.cache.block_count.load(Ordering::Relaxed), 5);

    // Receiving pruned block (open)
    let mut open1 = BlockBuilder::legacy_open()
        .source(send2.hash())
        .account(destination.account())
        .sign(&destination.key)
        .work(
            STUB_WORK_POOL
                .generate_dev2(destination.account().into())
                .unwrap(),
        )
        .build();
    ctx.ledger.process(txn.as_mut(), &mut open1).unwrap();

    // Rollback open block
    ctx.ledger.rollback(txn.as_mut(), &open1.hash()).unwrap();

    let info4 = ctx
        .ledger
        .pending_info(
            txn.txn(),
            &PendingKey::new(destination.account(), send2.hash()),
        )
        .unwrap();
    assert_ne!(info4.source, genesis.account()); // Tradeoff to not store pruned blocks accounts
    assert_eq!(info4.amount, Amount::raw(100));
    assert_eq!(info4.epoch, Epoch::Epoch0);

    // Process open block again
    ctx.ledger.process(txn.as_mut(), &mut open1).unwrap();
    assert_eq!(
        ctx.ledger.pending_info(
            txn.txn(),
            &PendingKey::new(destination.account(), send2.hash())
        ),
        None
    );
    assert_eq!(ctx.ledger.cache.pruned_count.load(Ordering::Relaxed), 2);
    assert_eq!(ctx.ledger.cache.block_count.load(Ordering::Relaxed), 6);
}

#[test]
fn pruning_process_error() {
    let ctx = LedgerContext::empty();
    ctx.ledger.enable_pruning();
    let genesis = ctx.genesis_block_factory();
    let mut txn = ctx.ledger.rw_txn();

    let mut send1 = genesis.send(txn.txn()).link(genesis.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    // Pruning action for latest block (not valid action)
    assert_eq!(ctx.ledger.pruning_action(txn.as_mut(), &send1.hash(), 1), 1);

    // Attempt to process pruned block again
    let result = ctx.ledger.process(txn.as_mut(), &mut send1).unwrap_err();
    assert_eq!(result, ProcessResult::Old);

    // Attept to process new block after pruned
    let mut send2 = BlockBuilder::state()
        .account(genesis.account())
        .previous(send1.hash())
        .balance(0)
        .link(genesis.account())
        .sign(&genesis.key)
        .work(STUB_WORK_POOL.generate_dev2(send1.hash().into()).unwrap())
        .build();
    let result = ctx.ledger.process(txn.as_mut(), &mut send2).unwrap_err();
    assert_eq!(result, ProcessResult::GapPrevious);
    assert_eq!(ctx.ledger.cache.pruned_count.load(Ordering::Relaxed), 1);
    assert_eq!(ctx.ledger.cache.block_count.load(Ordering::Relaxed), 2);
}

#[test]
fn pruning_legacy_blocks() {
    let ctx = LedgerContext::empty();
    ctx.ledger.enable_pruning();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = ctx.block_factory();

    let mut send1 = genesis
        .legacy_send(txn.txn())
        .destination(genesis.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let mut receive1 = genesis.legacy_receive(txn.txn(), send1.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut receive1).unwrap();

    let mut change1 = genesis
        .legacy_change(txn.txn())
        .representative(destination.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut change1).unwrap();

    let mut send2 = genesis
        .legacy_send(txn.txn())
        .destination(destination.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    let mut open1 = destination.legacy_open(send2.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open1).unwrap();

    let mut send3 = destination
        .legacy_send(txn.txn())
        .destination(genesis.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send3).unwrap();

    // Pruning action
    assert_eq!(
        ctx.ledger.pruning_action(txn.as_mut(), &change1.hash(), 2),
        3
    );

    assert_eq!(ctx.ledger.pruning_action(txn.as_mut(), &open1.hash(), 1), 1);

    assert!(ctx
        .ledger
        .store
        .block()
        .exists(txn.txn(), &DEV_GENESIS_HASH));
    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &send1.hash()),
        false
    );
    assert_eq!(
        ctx.ledger.store.pruned().exists(txn.txn(), &send1.hash()),
        true
    );
    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &receive1.hash()),
        false
    );
    assert_eq!(
        ctx.ledger
            .store
            .pruned()
            .exists(txn.txn(), &receive1.hash()),
        true
    );
    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &change1.hash()),
        false
    );
    assert_eq!(
        ctx.ledger.store.pruned().exists(txn.txn(), &change1.hash()),
        true
    );
    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &send2.hash()),
        true
    );
    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &open1.hash()),
        false
    );
    assert_eq!(
        ctx.ledger.store.pruned().exists(txn.txn(), &open1.hash()),
        true
    );
    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &send3.hash()),
        true
    );
    assert_eq!(ctx.ledger.cache.pruned_count.load(Ordering::Relaxed), 4);
    assert_eq!(ctx.ledger.cache.block_count.load(Ordering::Relaxed), 7);
    assert_eq!(ctx.ledger.store.pruned().count(txn.txn()), 4);
    assert_eq!(ctx.ledger.store.block().count(txn.txn()), 3);
}

#[test]
fn pruning_safe_functions() {
    let ctx = LedgerContext::empty();
    ctx.ledger.enable_pruning();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut send1 = genesis.send(txn.txn()).link(genesis.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let mut send2 = genesis.send(txn.txn()).link(genesis.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    // Pruning action
    assert_eq!(ctx.ledger.pruning_action(txn.as_mut(), &send1.hash(), 1), 1);

    // Safe ledger actions
    assert!(ctx.ledger.balance_safe(txn.txn(), &send1.hash()).is_err());
    assert_eq!(
        ctx.ledger.balance_safe(txn.txn(), &send2.hash()).unwrap(),
        send2.balance()
    );

    assert_eq!(ctx.ledger.amount_safe(txn.txn(), &send2.hash()), None);
    assert_eq!(ctx.ledger.account(txn.txn(), &send1.hash()), None);
    assert_eq!(
        ctx.ledger.account(txn.txn(), &send2.hash()),
        Some(genesis.account())
    );
}

#[test]
fn hash_root_random() {
    let ctx = LedgerContext::empty();
    ctx.ledger.enable_pruning();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut send1 = genesis.send(txn.txn()).link(genesis.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let mut send2 = genesis.send(txn.txn()).link(genesis.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    // Pruning action
    assert_eq!(ctx.ledger.pruning_action(txn.as_mut(), &send1.hash(), 1), 1);

    // Test random block including pruned
    let mut done = false;
    let mut iteration = 0;
    while !done {
        iteration += 1;
        let root_hash = ctx.ledger.hash_root_random(txn.txn()).unwrap();
        done = (root_hash.0 == send1.hash()) && root_hash.1.is_zero();
        assert!(iteration < 1000);
    }
}
