use rsnano_core::{Amount, BlockEnum, KeyPair, StateBlock, DEV_GENESIS_KEY};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
use test_helpers::{start_election, System};

#[test]
fn start_stop() {
    let mut system = System::new();
    let node1 = system.make_node();
    let key1 = KeyPair::new();
    let send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::zero(),
        key1.public_key().as_account().into(),
        &DEV_GENESIS_KEY,
        node1.work_generate_dev((*DEV_GENESIS_HASH).into()),
    ));
    node1.process(send1.clone()).unwrap();
    assert_eq!(node1.active.len(), 0);
    let election1 = start_election(&node1, &send1.hash());
    assert_eq!(node1.active.len(), 1);
    assert_eq!(election1.vote_count(), 1);
}
