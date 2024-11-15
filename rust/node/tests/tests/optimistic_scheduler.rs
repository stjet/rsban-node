use std::time::Duration;

use rsnano_core::DEV_GENESIS_KEY;
use rsnano_node::consensus::ElectionBehavior;
use test_helpers::{assert_timely, setup_chains, System};

/*
 * Ensure account gets activated for a single unconfirmed account chain
 */
#[test]
pub fn activate_one() {
    let mut system = System::new();
    let node = system.make_node();

    // Needs to be greater than optimistic scheduler `gap_threshold`
    let howmany_blocks = 64;

    let chains = setup_chains(
        &node,
        /* single chain */ 1,
        howmany_blocks,
        &DEV_GENESIS_KEY,
        /* do not confirm */ false,
    );
    let (_, blocks) = chains.first().unwrap();

    // Confirm block towards at the beginning the chain, so gap between confirmation
    // and account frontier is larger than `gap_threshold`
    node.confirm(blocks[11].hash());

    // Ensure unconfirmed account head block gets activated
    let block = blocks.last().unwrap();
    assert_timely(Duration::from_secs(5), || {
        node.vote_router.active(&block.hash())
    });
    assert_eq!(
        node.active
            .election(&block.qualified_root())
            .unwrap()
            .behavior,
        ElectionBehavior::Optimistic
    );
}
