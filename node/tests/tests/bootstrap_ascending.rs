use rsnano_core::{Account, PrivateKey, UnsavedBlockLatticeBuilder};
use std::time::Duration;
use test_helpers::{assert_timely, System};

/**
 * Tests the base case for returning
 */
#[test]
fn account_base() {
    let mut system = System::new();
    let node0 = system.make_node();
    let mut lattice = UnsavedBlockLatticeBuilder::new();
    let send1 = lattice.genesis().send(Account::zero(), 1);
    node0.process(send1.clone()).unwrap();
    let node1 = system.make_node();
    assert_timely(Duration::from_secs(5), || node1.block_exists(&send1.hash()));
}

/**
 * Tests that bootstrap_ascending will return multiple new blocks in-order
 */
#[test]
fn account_inductive() {
    let mut system = System::new();
    let node0 = system.make_node();
    let mut lattice = UnsavedBlockLatticeBuilder::new();
    let send1 = lattice.genesis().send(Account::zero(), 1);
    let send2 = lattice.genesis().send(Account::zero(), 1);
    node0.process(send1).unwrap();
    node0.process(send2.clone()).unwrap();
    let node1 = system.make_node();
    assert_timely(Duration::from_secs(50), || {
        node1.block_exists(&send2.hash())
    });
}

/**
 * Tests that bootstrap_ascending will return multiple new blocks in-order
 */

#[test]
fn trace_base() {
    let mut system = System::new();
    let node0 = system.make_node();
    let mut lattice = UnsavedBlockLatticeBuilder::new();
    let key = PrivateKey::new();
    let send1 = lattice.genesis().send(&key, 1);
    let receive1 = lattice.account(&key).receive(&send1);
    node0.process(send1).unwrap();
    node0.process(receive1.clone()).unwrap();
    let node1 = system.make_node();
    assert_timely(Duration::from_secs(10), || {
        node1.block_exists(&receive1.hash())
    });
}
