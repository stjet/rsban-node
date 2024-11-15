use std::time::Duration;

use rsnano_core::{Amount, BlockEnum, BlockSideband, KeyPair, StateBlock, DEV_GENESIS_KEY};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
use rsnano_network::ChannelId;
use rsnano_node::block_processing::BlockSource;
use test_helpers::{assert_timely, start_election, System};

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

#[test]
fn add_existing() {
    let mut system = System::new();
    let node1 = system.make_node();
    let key1 = KeyPair::new();

    // create a send block to send all of the nano supply to key1
    let send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::zero(),
        key1.public_key().as_account().into(),
        &DEV_GENESIS_KEY,
        node1.work_generate_dev((*DEV_GENESIS_HASH).into()),
    ));

    // add the block to ledger as an unconfirmed block
    node1.process(send1.clone()).unwrap();

    // instruct the election scheduler to trigger an election for send1
    start_election(&node1, &send1.hash());

    // wait for election to be started before processing send2
    assert_timely(Duration::from_secs(5), || node1.active.active(&send1));

    let key2 = KeyPair::new();
    let mut send2 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::zero(),
        key2.public_key().as_account().into(),
        &DEV_GENESIS_KEY,
        node1.work_generate_dev((*DEV_GENESIS_HASH).into()),
    ));
    send2.set_sideband(BlockSideband::new_test_instance());

    // the block processor will notice that the block is a fork and it will try to publish it
    // which will update the election object
    node1
        .block_processor
        .add(send2.clone().into(), BlockSource::Live, ChannelId::LOOPBACK);

    assert!(node1.active.active(&send1));
    assert_timely(Duration::from_secs(5), || node1.active.active(&send2));
}
