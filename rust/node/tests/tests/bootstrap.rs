use std::time::Duration;

use rsnano_core::{Amount, BlockEnum, BlockHash, KeyPair, StateBlock, DEV_GENESIS_KEY};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH};
use rsnano_node::{
    bootstrap::{BootstrapInitiatorExt, BootstrapStrategy},
    config::{FrontiersConfirmationMode, NodeFlags},
};

use super::helpers::{assert_timely, assert_timely_eq, establish_tcp, System};

#[test]
fn bootstrap_processor_lazy_hash() {
    let mut system = System::new();
    let mut config = System::default_config();
    config.frontiers_confirmation = FrontiersConfirmationMode::Disabled;
    let mut flags = NodeFlags::new();
    flags.disable_bootstrap_bulk_push_client = true;
    let node0 = system.build_node().config(config).flags(flags).finish();

    let key1 = KeyPair::new();
    let key2 = KeyPair::new();

    let send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_ACCOUNT,
        Amount::MAX - Amount::nano(1000),
        key1.public_key().into(),
        &DEV_GENESIS_KEY,
        node0.work_generate_dev((*DEV_GENESIS_HASH).into()),
    ));

    let receive1 = BlockEnum::State(StateBlock::new(
        key1.public_key(),
        BlockHash::zero(),
        key1.public_key(),
        Amount::nano(1000),
        send1.hash().into(),
        &key1,
        node0.work_generate_dev(key1.public_key().into()),
    ));

    let send2 = BlockEnum::State(StateBlock::new(
        key1.public_key(),
        receive1.hash(),
        key1.public_key(),
        Amount::zero(),
        key2.public_key().into(),
        &key1,
        node0.work_generate_dev(receive1.hash().into()),
    ));

    let receive2 = BlockEnum::State(StateBlock::new(
        key2.public_key(),
        BlockHash::zero(),
        key2.public_key(),
        Amount::nano(1000),
        send2.hash().into(),
        &key2,
        node0.work_generate_dev(key2.public_key().into()),
    ));

    // Processing test chain
    node0.process_local(send1.clone());
    node0.process_local(receive1.clone());
    node0.process_local(send2.clone());
    node0.process_local(receive2.clone());

    assert_timely(
        Duration::from_secs(5),
        || node0.blocks_exist(&[send1.hash(), receive1.hash(), send2.hash(), receive2.hash()]),
        "blocks not processed",
    );

    // Start lazy bootstrap with last block in chain known
    let node1 = system.make_disconnected_node();
    establish_tcp(&node1, &node0);
    node1
        .bootstrap_initiator
        .bootstrap_lazy(receive2.hash().into(), true, "".to_string());

    {
        let lazy_attempt = node1
            .bootstrap_initiator
            .current_lazy_attempt()
            .expect("no lazy attempt found");

        let BootstrapStrategy::Lazy(lazy) = lazy_attempt.as_ref() else {
            panic!("not lazy")
        };
        assert_eq!(lazy.attempt.id, receive2.hash().to_string());
    }

    // Check processed blocks
    assert_timely_eq(
        Duration::from_secs(10),
        || node1.balance(&key2.public_key()),
        Amount::nano(1000),
    );
}
