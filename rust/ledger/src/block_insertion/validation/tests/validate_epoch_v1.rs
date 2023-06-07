use rsnano_core::{Account, AccountInfo, Amount, BlockDetails, BlockHash, Epoch};

use crate::{
    block_insertion::validation::tests::{
        assert_block_is_valid, create_epoch1_open_block, create_legacy_open_block,
        create_validator_for_existing_account, epoch_successor, legacy_receive_successor,
        setup_pending_receive,
    },
    ProcessResult,
};

use super::{
    assert_validation_fails_with, create_state_block, create_test_validator,
    legacy_change_successor, state_successor,
};

#[test]
fn updgrade_to_epoch_v1() {
    let (_, previous) = create_legacy_open_block();
    let epoch = epoch_successor(&previous, Epoch::Epoch1).build();
    let instructions = assert_block_is_valid(&epoch, Some(previous));
    assert_eq!(instructions.set_account_info.epoch, Epoch::Epoch1);
    assert_eq!(
        instructions.set_sideband.details,
        BlockDetails::new(Epoch::Epoch1, false, false, true)
    );
    assert_eq!(instructions.set_sideband.source_epoch, Epoch::Epoch0); // source_epoch is not used for epoch blocks
}

#[test]
fn adding_epoch_twice_fails() {
    let (_, previous) = create_state_block(Epoch::Epoch1);
    let epoch = epoch_successor(&previous, Epoch::Epoch1).build();
    assert_validation_fails_with(ProcessResult::BlockPosition, &epoch, Some(previous));
}

#[test]
fn adding_legacy_change_block_after_epoch1_fails() {
    let (keypair, previous) = create_state_block(Epoch::Epoch1);
    let change = legacy_change_successor(keypair, &previous).build();
    assert_validation_fails_with(ProcessResult::BlockPosition, &change, Some(previous));
}

#[test]
fn can_add_state_blocks_after_epoch1() {
    let (keypair, previous) = create_state_block(Epoch::Epoch1);
    let state = state_successor(keypair, &previous).build();
    assert_block_is_valid(&state, Some(previous));
}

#[test]
fn epoch_block_with_changed_representative_fails() {
    let (_, open) = create_legacy_open_block();
    let epoch_with_invalid_rep = epoch_successor(&open, Epoch::Epoch1)
        .representative(Account::from(999999))
        .build();
    assert_validation_fails_with(
        ProcessResult::RepresentativeMismatch,
        &epoch_with_invalid_rep,
        Some(open),
    );
}

#[test]
fn cannot_use_legacy_open_block_if_sender_is_on_epoch1() {
    let (_, legacy_open) = create_legacy_open_block();

    let mut validator = create_test_validator(&legacy_open, legacy_open.account());
    setup_pending_receive(&mut validator, Epoch::Epoch1, Amount::raw(10));

    let result = validator.validate();
    assert_eq!(result, Err(ProcessResult::Unreceivable));
}

#[test]
fn cannot_use_legacy_receive_block_after_epoch1_open() {
    let (keypair, previous) = create_state_block(Epoch::Epoch1);
    let legacy_receive = legacy_receive_successor(keypair, &previous).build();
    let mut validator = create_validator_for_existing_account(&legacy_receive, previous);
    setup_pending_receive(&mut validator, Epoch::Epoch0, Amount::raw(10));

    let result = validator.validate();

    assert_eq!(result, Err(ProcessResult::BlockPosition));
}

#[test]
fn cannot_use_legacy_receive_block_after_sender_upgraded_to_epoch1() {
    let (keypair, previous) = create_legacy_open_block();
    let legacy_receive = legacy_receive_successor(keypair, &previous).build();
    let mut validator = create_validator_for_existing_account(&legacy_receive, previous);
    setup_pending_receive(&mut validator, Epoch::Epoch1, Amount::raw(10));

    let result = validator.validate();

    assert_eq!(result, Err(ProcessResult::Unreceivable));
}

#[test]
fn can_add_state_receive_block_after_epoch1() {
    let (keypair, previous) = create_state_block(Epoch::Epoch1);

    let state_receive = state_successor(keypair, &previous)
        .link(123)
        .balance(previous.balance() + Amount::raw(10))
        .build();

    let mut validator = create_validator_for_existing_account(&state_receive, previous);
    setup_pending_receive(&mut validator, Epoch::Epoch1, Amount::raw(10));

    let result = validator.validate().expect("block should be valid");
    assert_eq!(result.set_sideband.details.epoch, Epoch::Epoch1);
    assert_eq!(result.set_sideband.source_epoch, Epoch::Epoch1);
}

#[test]
fn can_open_account_with_epoch1_block() {
    let epoch1_open = create_epoch1_open_block();
    let mut validator = create_test_validator(&epoch1_open, epoch1_open.account());
    validator.any_pending_exists = true;

    let result = validator.validate().expect("block should be valid");

    assert_eq!(
        result.set_account_info,
        AccountInfo {
            head: epoch1_open.hash(),
            representative: epoch1_open.representative().unwrap(),
            open_block: epoch1_open.hash(),
            balance: Amount::zero(),
            modified: validator.seconds_since_epoch,
            block_count: 1,
            epoch: Epoch::Epoch1
        }
    )
}

#[test]
fn receiving_from_epoch1_sender_upgrades_receiver_to_epoch1() {
    let (keypair, previous) = create_legacy_open_block();
    let receive = state_successor(keypair, &previous).link(123).build();

    let mut validator = create_validator_for_existing_account(&receive, previous);
    setup_pending_receive(&mut validator, Epoch::Epoch1, Amount::raw(10));

    let result = validator.validate().expect("block should be valid");
    assert_eq!(result.set_account_info.epoch, Epoch::Epoch1);
    assert_eq!(result.set_sideband.details.epoch, Epoch::Epoch1);
}

#[test]
fn epoch_v1_fork() {
    let (_, previous) = create_legacy_open_block();
    let epoch1_block = epoch_successor(&previous, Epoch::Epoch1).build();
    let mut validator = create_validator_for_existing_account(&epoch1_block, previous);
    validator.old_account_info = Some(AccountInfo {
        epoch: Epoch::Epoch0,
        head: BlockHash::from(123),
        ..AccountInfo::create_test_instance()
    });

    let result = validator.validate();

    assert_eq!(result, Err(ProcessResult::Fork));
}
