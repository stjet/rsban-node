use rsnano_core::{Amount, BlockEnum, KeyPair, StateBlock, DEV_GENESIS_KEY};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_PUB_KEY};
use rsnano_node::stats::{DetailType, Direction, StatType};
use test_helpers::System;

#[test]
fn single() {
    let amount = Amount::MAX;
    let mut system = System::new();
    let node = system.make_node();
    let key1 = KeyPair::new();
    node.insert_into_wallet(&DEV_GENESIS_KEY);
    let latest1 = node.latest(&DEV_GENESIS_ACCOUNT);
    let send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        latest1,
        *DEV_GENESIS_PUB_KEY,
        amount - Amount::raw(100),
        key1.public_key().as_account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(latest1.into()),
    ));
    node.process(send1.clone()).unwrap();
    let mut tx = node.ledger.rw_txn();
    assert_eq!(
        node.ledger.confirmed().block_exists(&tx, &send1.hash()),
        false
    );
    node.ledger.confirm(&mut tx, send1.hash());
    assert_eq!(
        node.ledger.confirmed().block_exists(&tx, &send1.hash()),
        true
    );
    let conf_height = node
        .ledger
        .get_confirmation_height(&tx, &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(conf_height.height, 2);
    assert_eq!(conf_height.frontier, send1.hash());

    // Rollbacks should fail as these blocks have been cemented
    assert!(node.ledger.rollback(&mut tx, &latest1).is_err());
    assert!(node.ledger.rollback(&mut tx, &send1.hash()).is_err());
    assert_eq!(
        node.stats.count(
            StatType::ConfirmationHeight,
            DetailType::BlocksConfirmed,
            Direction::In
        ),
        1
    );
    assert_eq!(node.ledger.cemented_count(), 2);
}
