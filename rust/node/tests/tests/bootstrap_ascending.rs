use std::time::Duration;

use rsnano_core::{Amount, BlockEnum, StateBlock, DEV_GENESIS_KEY};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
use test_helpers::{assert_timely, System};

/**
 * Tests the base case for returning
 */
#[test]
fn account_base() {
    let mut system = System::new();
    let node0 = system.make_node();
    let send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::raw(1),
        0.into(),
        &DEV_GENESIS_KEY,
        node0.work_generate_dev((*DEV_GENESIS_HASH).into()),
    ));
    node0.process(send1.clone()).unwrap();
    let node1 = system.make_node();
    assert_timely(Duration::from_secs(5), || node1.block_exists(&send1.hash()));
}
