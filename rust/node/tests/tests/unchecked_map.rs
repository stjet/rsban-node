use rsnano_core::{Amount, BlockEnum, StateBlock, UncheckedInfo, UncheckedKey, DEV_GENESIS_KEY};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
use rsnano_node::{block_processing::UncheckedMap, stats::Stats};
use std::{sync::Arc, time::Duration};
use test_helpers::assert_timely;

#[test]
fn one_bootstrap() {
    let unchecked = UncheckedMap::new(65536, Arc::new(Stats::default()), false);
    let block1 = Arc::new(BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::raw(1),
        (*DEV_GENESIS_ACCOUNT).into(),
        &DEV_GENESIS_KEY,
        0,
    )));
    unchecked.put(block1.hash().into(), UncheckedInfo::new(block1.clone()));

    // Waits for the block1 to get saved in the database
    assert_timely(Duration::from_secs(10), || {
        unchecked.get(&block1.hash().into()).len() > 0
    });
    let mut dependencies = Vec::new();
    unchecked.for_each(
        |key, _| {
            dependencies.push(key.hash);
        },
        || true,
    );
    let hash1 = dependencies[0];
    assert_eq!(block1.hash(), hash1);
    let mut blocks = unchecked.get(&hash1.into());
    assert_eq!(blocks.len(), 1);
    let block2 = blocks.remove(0).block.unwrap();
    assert_eq!(block2.hash(), block1.hash());
}

// This test checks for basic operations in the unchecked table such as putting a new block, retrieving it, and
// deleting it from the database
#[test]
fn simple() {
    let unchecked = UncheckedMap::new(65536, Arc::new(Stats::default()), false);
    let block = Arc::new(BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::raw(1),
        (*DEV_GENESIS_ACCOUNT).into(),
        &DEV_GENESIS_KEY,
        0,
    )));
    // Asserts the block wasn't added yet to the unchecked table
    let block_listing1 = unchecked.get(&block.previous().into());
    assert!(block_listing1.is_empty());
    // Enqueues a block to be saved on the unchecked table
    unchecked.put(block.previous().into(), UncheckedInfo::new(block.clone()));
    // Waits for the block to get written in the database
    assert_timely(Duration::from_secs(5), || {
        unchecked.get(&block.previous().into()).len() > 0
    });
    // Retrieves the block from the database
    let block_listing2 = unchecked.get(&block.previous().into());
    assert_ne!(block_listing2.len(), 0);
    // Asserts the added block is equal to the retrieved one
    assert_eq!(
        block_listing2[0].block.as_ref().unwrap().hash(),
        block.hash()
    );
    // Deletes the block from the database
    unchecked.remove(&UncheckedKey::new(block.previous(), block.hash()));
    // Asserts the block is deleted
    let block_listing3 = unchecked.get(&block.previous().into());
    assert!(block_listing3.is_empty());
}
