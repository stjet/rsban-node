use rsnano_core::{Amount, BlockEnum, StateBlock, UncheckedInfo, DEV_GENESIS_KEY};
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
