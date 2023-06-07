use rsnano_core::{
    work::WORK_THRESHOLDS_STUB, Account, AccountInfo, Amount, BlockBuilder, BlockDetails,
    BlockHash, BlockSideband, Epoch, Epochs, KeyPair, LegacySendBlockBuilder, PendingInfo,
    PendingKey,
};

use crate::{
    block_insertion::{validation::tests::create_test_account_info, BlockValidator},
    ProcessResult,
};

use super::ValidateOutput;

#[test]
fn valid_legacy_send_block() {
    let output = validate_legacy_send_block(Default::default());

    let result = output.result.unwrap();
    assert_eq!(result.account, output.account);
    assert_eq!(
        result.set_sideband,
        BlockSideband {
            height: output.old_account_info.block_count + 1,
            timestamp: output.seconds_since_epoch,
            successor: BlockHash::zero(),
            account: output.account,
            balance: Amount::raw(900),
            details: BlockDetails::new(Epoch::Epoch0, true, false, false),
            source_epoch: Epoch::Epoch0
        }
    );
    assert_eq!(result.delete_pending, None);
    assert_eq!(
        result.insert_pending,
        Some((
            PendingKey::new(output.block.destination_or_link(), output.block.hash()),
            PendingInfo {
                source: output.account,
                amount: Amount::raw(100),
                epoch: Epoch::Epoch0,
            }
        ))
    );
    assert_eq!(
        result.set_account_info,
        AccountInfo {
            head: output.block.hash(),
            representative: output.old_account_info.representative,
            open_block: output.old_account_info.open_block,
            balance: output.block.balance(),
            modified: output.seconds_since_epoch,
            block_count: output.old_account_info.block_count + 1,
            epoch: Epoch::Epoch0
        }
    );
}

#[test]
fn fails_with_old_if_legacy_sending_twice() {
    let output = validate_legacy_send_block(ValidateLegacySendBlockOptions {
        setup_validator: Some(&mut |validator| validator.block_exists = true),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::Old));
}

#[test]
fn fails_with_fork_if_legacy_send_block_has_unexpected_previous_block() {
    let output = validate_legacy_send_block(ValidateLegacySendBlockOptions {
        setup_block: Some(&|block| block.previous(BlockHash::from(99999))),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::Fork));
}

#[test]
fn legacy_send_fails_with_gap_previous_if_account_not_found() {
    let output = validate_legacy_send_block(ValidateLegacySendBlockOptions {
        setup_validator: Some(&mut |validator| {
            validator.previous_block = None;
            validator.old_account_info = None;
            validator.account = Account::zero();
        }),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::GapPrevious));
}

#[test]
fn fail_if_signature_is_bad() {
    let output = validate_legacy_send_block(ValidateLegacySendBlockOptions {
        setup_block: Some(&|block| block.sign(KeyPair::new())),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::BadSignature));
}

#[test]
fn fails_if_sending_negative_amount() {
    let output = validate_legacy_send_block(ValidateLegacySendBlockOptions {
        setup_block: Some(&|block| block.balance(Amount::raw(99999))),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::NegativeSpend));
}

#[test]
fn send_fails_if_work_is_insufficient_for_epoch_0() {
    let output = validate_legacy_send_block(ValidateLegacySendBlockOptions {
        setup_block: Some(&|block| block.work(0)),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::InsufficientWork));
}

#[test]
fn fails_if_legacy_send_follows_a_state_block() {
    let keypair = KeyPair::new();

    let mut previous = BlockBuilder::state()
        .account(keypair.public_key())
        .with_sideband()
        .build();
    previous.set_sideband(BlockSideband {
        successor: BlockHash::zero(),
        account: keypair.public_key(),
        balance: Amount::raw(1000),
        details: BlockDetails::new(Epoch::Epoch2, false, true, false),
        ..BlockSideband::create_test_instance()
    });

    let legacy_send = BlockBuilder::legacy_send()
        .destination(Account::from(12345))
        .previous(previous.hash())
        .balance(Amount::raw(900))
        .sign(keypair.clone())
        .build();
    let old_account_info = Some(create_test_account_info(&previous));
    let validator = BlockValidator {
        block: &legacy_send,
        epochs: &Epochs::new(),
        work: &WORK_THRESHOLDS_STUB,
        block_exists: false,
        account: keypair.public_key(),
        frontier_missing: false,
        previous_block: Some(previous),
        old_account_info,
        pending_receive_info: None,
        any_pending_exists: false,
        source_block_exists: false,
        seconds_since_epoch: 123456,
    };

    let result = validator.validate();
    assert_eq!(result, Err(ProcessResult::BlockPosition))
}

struct ValidateLegacySendBlockOptions<'a> {
    setup_block: Option<&'a dyn Fn(LegacySendBlockBuilder) -> LegacySendBlockBuilder>,
    setup_validator: Option<&'a mut dyn FnMut(&mut BlockValidator)>,
}

impl<'a> Default for ValidateLegacySendBlockOptions<'a> {
    fn default() -> Self {
        Self {
            setup_block: None,
            setup_validator: None,
        }
    }
}

fn validate_legacy_send_block(mut options: ValidateLegacySendBlockOptions) -> ValidateOutput {
    let keypair = KeyPair::new();

    let mut previous = BlockBuilder::legacy_open()
        .account(keypair.public_key())
        .with_sideband()
        .build();
    previous.set_sideband(BlockSideband {
        successor: BlockHash::zero(),
        account: keypair.public_key(),
        balance: Amount::raw(1000),
        details: BlockDetails::new(Epoch::Epoch0, false, true, false),
        ..BlockSideband::create_test_instance()
    });

    let mut send = BlockBuilder::legacy_send()
        .destination(Account::from(12345))
        .previous(previous.hash())
        .balance(Amount::raw(900))
        .sign(keypair.clone());

    if let Some(setup) = &options.setup_block {
        send = setup(send);
    }
    let send = send.build();
    let old_account_info = create_test_account_info(&previous);

    let mut validator = BlockValidator {
        block: &send,
        epochs: &Epochs::new(),
        work: &WORK_THRESHOLDS_STUB,
        block_exists: false,
        account: keypair.public_key(),
        frontier_missing: false,
        previous_block: Some(previous),
        old_account_info: Some(old_account_info.clone()),
        pending_receive_info: None,
        any_pending_exists: false,
        source_block_exists: false,
        seconds_since_epoch: 123456,
    };
    if let Some(setup) = &mut options.setup_validator {
        setup(&mut validator);
    }

    let result = validator.validate();
    ValidateOutput {
        seconds_since_epoch: validator.seconds_since_epoch,
        result,
        block: send,
        old_account_info,
        account: keypair.public_key(),
    }
}
