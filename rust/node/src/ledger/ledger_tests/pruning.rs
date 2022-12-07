use std::sync::atomic::Ordering;

use rsnano_core::{
    work::DEV_WORK_POOL, Amount, Block, BlockBuilder, BlockDetails, BlockEnum, Epoch, PendingKey,
};

use crate::{ledger::ledger_tests::LedgerContext, DEV_CONSTANTS, DEV_GENESIS_HASH};

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
        .balance(DEV_CONSTANTS.genesis_amount - Amount::new(100))
        .link(send1.hash())
        .sign(&genesis.key)
        .work(DEV_WORK_POOL.generate_dev2(send2.hash().into()).unwrap())
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
    let BlockEnum::State(receive1_stored) = ctx.ledger.get_block(txn.txn(), &receive1.hash()).unwrap() else { panic!("not a state block")};
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
