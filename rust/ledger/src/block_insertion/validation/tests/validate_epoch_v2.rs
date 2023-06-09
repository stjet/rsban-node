use super::{
    assert_block_is_valid, assert_validation_fails_with, create_test_validator,
    create_validator_for_existing_account, setup_pending_receive,
};
use crate::{
    test_helpers::{
        create_legacy_open_block, create_state_block, epoch_successor, legacy_receive_successor,
        state_successor,
    },
    ProcessResult,
};
use rsnano_core::{Amount, BlockBuilder, Epoch, KeyPair};

#[test]
fn fails_if_directly_upgrading_from_epoch_0_to_epoch_2() {
    let (_, previous) = create_legacy_open_block();
    let epoch = epoch_successor(&previous, Epoch::Epoch2).build();
    // Trying to upgrade from epoch 0 to epoch 2. It is a requirement epoch upgrades are sequential unless the account is unopened
    assert_validation_fails_with(ProcessResult::BlockPosition, &epoch, Some(previous));
}

#[test]
fn upgrade_from_epoch_1_to_epoch_2() {
    let (_, previous) = create_state_block(Epoch::Epoch1);
    let epoch = epoch_successor(&previous, Epoch::Epoch2).build();
    let instructions = assert_block_is_valid(&epoch, Some(previous));
    assert_eq!(instructions.set_account_info.epoch, Epoch::Epoch2);
    assert_eq!(instructions.set_sideband.details.epoch, Epoch::Epoch2);
}

#[test]
fn upgrading_to_epoch_v2_twice_fails() {
    let (_, previous) = create_state_block(Epoch::Epoch2);
    let epoch = epoch_successor(&previous, Epoch::Epoch2).build();
    assert_validation_fails_with(ProcessResult::BlockPosition, &epoch, Some(previous));
}

#[test]
fn legacy_receive_block_after_epoch_v2_upgrade_fails() {
    let (keypair, previous) = create_state_block(Epoch::Epoch2);
    let legacy_receive = legacy_receive_successor(keypair, &previous).build();
    assert_validation_fails_with(
        ProcessResult::BlockPosition,
        &legacy_receive,
        Some(previous),
    );
}

#[test]
fn cannot_use_legacy_open_block_with_epoch_v2_send() {
    let (keypair, legacy_open) = create_legacy_open_block();
    let mut validator = create_test_validator(&legacy_open, keypair.public_key());
    setup_pending_receive(&mut validator, Epoch::Epoch2, Amount::raw(10));
    let result = validator.validate();
    assert_eq!(result, Err(ProcessResult::Unreceivable));
}

#[test]
fn receive_after_epoch_v2_upgrade() {
    let (keypair, previous) = create_state_block(Epoch::Epoch2);
    let receive = state_successor(keypair, &previous)
        .link(123)
        .balance(previous.balance() - Amount::raw(10))
        .build();

    let mut validator = create_validator_for_existing_account(&receive, previous);
    setup_pending_receive(&mut validator, Epoch::Epoch2, Amount::raw(10));

    validator.validate().expect("block should be valid");
}

#[test]
fn receiving_from_epoch_2_block_upgrades_receiver_to_epoch2() {
    let (keypair, previous) = create_legacy_open_block();
    let receive = state_successor(keypair, &previous).link(123).build();

    let mut validator = create_validator_for_existing_account(&receive, previous);
    setup_pending_receive(&mut validator, Epoch::Epoch2, Amount::raw(10));
    let instructions = validator.validate().expect("block should be valid");

    assert_eq!(instructions.set_account_info.epoch, Epoch::Epoch2);
    assert_eq!(instructions.set_sideband.details.epoch, Epoch::Epoch2);
}

#[test]
fn upgrade_new_account_straight_to_epoch_2() {
    let keypair = KeyPair::new();

    let open = BlockBuilder::state()
        .account(keypair.public_key())
        .previous(0)
        .balance(10)
        .sign(&keypair)
        .build();

    let mut validator = create_test_validator(&open, keypair.public_key());
    setup_pending_receive(&mut validator, Epoch::Epoch2, open.balance());

    let instructions = validator.validate().expect("block should be valid");
    assert_eq!(instructions.set_account_info.epoch, Epoch::Epoch2);
    assert_eq!(instructions.set_sideband.details.epoch, Epoch::Epoch2);
}
