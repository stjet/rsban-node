use rsnano_core::{
    work::WORK_THRESHOLDS_STUB, Account, Amount, BlockBuilder, BlockDetails, BlockHash,
    BlockSideband, Epoch, Epochs, KeyPair, LegacyReceiveBlockBuilder, PendingInfo, PendingKey,
};

use crate::{
    block_insertion::{validation::tests::create_account_info, BlockValidator},
    ProcessResult,
};

use super::ValidateOutput;

#[test]
fn valid_legacy_receive_block() {
    let output = validate_legacy_receive_block(Default::default());

    let result = output.result.unwrap();
    assert_eq!(result.account, output.account);
    assert_eq!(
        result.set_sideband,
        BlockSideband {
            height: output.old_account_info.block_count + 1,
            timestamp: output.seconds_since_epoch,
            successor: BlockHash::zero(),
            account: output.account,
            balance: Amount::raw(1010),
            details: BlockDetails::new(Epoch::Epoch0, false, true, false),
            source_epoch: Epoch::Epoch0,
        }
    );
    assert_eq!(
        result.delete_pending,
        Some(PendingKey::new(
            output.account,
            output.block.source_or_link()
        ))
    );
    assert_eq!(result.insert_pending, None);
}

#[test]
fn fails_with_fork_if_already_received() {
    let output = validate_legacy_receive_block(ValidateLegacyReceiveBlockOptions {
        setup_block: Some(&|block| block.previous(BlockHash::from(123))),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::Fork));
}

#[test]
fn fails_with_unreceivable_if_legacy_send_already_received() {
    let output = validate_legacy_receive_block(ValidateLegacyReceiveBlockOptions {
        setup_validator: Some(&mut |valdiator| {
            valdiator.pending_receive_info = None;
        }),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::Unreceivable));
}

#[test]
fn fails_with_gap_source_if_legacy_source_not_found() {
    let output = validate_legacy_receive_block(ValidateLegacyReceiveBlockOptions {
        setup_validator: Some(&mut |valdiator| {
            valdiator.source_block_exists = false;
            valdiator.pending_receive_info = None;
        }),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::GapSource));
}

#[test]
fn fails_if_legacy_receive_has_bad_signature() {
    let output = validate_legacy_receive_block(ValidateLegacyReceiveBlockOptions {
        setup_block: Some(&|block| block.sign(&KeyPair::new())),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::BadSignature));
}

#[test]
fn fails_if_previous_missing() {
    let output = validate_legacy_receive_block(ValidateLegacyReceiveBlockOptions {
        setup_validator: Some(&mut |valdiator| {
            valdiator.previous_block = None;
        }),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::GapPrevious));
}

#[test]
fn fails_if_previous_missing_and_account_unknown() {
    let output = validate_legacy_receive_block(ValidateLegacyReceiveBlockOptions {
        setup_validator: Some(&mut |valdiator| {
            valdiator.old_account_info = None;
            valdiator.previous_block = None;
        }),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::GapPrevious));
}

#[test]
fn fails_if_legacy_receive_follows_state_block() {
    let keypair = KeyPair::new();

    let mut previous = BlockBuilder::state()
        .account(keypair.public_key())
        .with_sideband()
        .build();
    previous.set_sideband(BlockSideband {
        successor: BlockHash::zero(),
        account: keypair.public_key(),
        details: BlockDetails::new(Epoch::Epoch2, false, true, false),
        ..BlockSideband::create_test_instance()
    });

    let legacy_receive = BlockBuilder::legacy_receive()
        .previous(previous.hash())
        .source(BlockHash::from(42))
        .sign(&keypair)
        .build();
    let old_account_info = Some(create_account_info(&previous));
    let validator = BlockValidator {
        block: &legacy_receive,
        epochs: &Epochs::new(),
        work: &WORK_THRESHOLDS_STUB,
        block_exists: false,
        account: keypair.public_key(),
        frontier_missing: false,
        previous_block: Some(previous),
        old_account_info,
        pending_receive_info: Some(PendingInfo::new(
            Account::from(123),
            Amount::raw(100),
            Epoch::Epoch0,
        )),
        any_pending_exists: true,
        source_block_exists: true,
        seconds_since_epoch: 123456,
    };

    let result = validator.validate();
    assert_eq!(result, Err(ProcessResult::BlockPosition))
}

struct ValidateLegacyReceiveBlockOptions<'a> {
    setup_block: Option<&'a dyn Fn(LegacyReceiveBlockBuilder) -> LegacyReceiveBlockBuilder>,
    setup_validator: Option<&'a mut dyn FnMut(&mut BlockValidator)>,
}

impl<'a> Default for ValidateLegacyReceiveBlockOptions<'a> {
    fn default() -> Self {
        Self {
            setup_block: None,
            setup_validator: None,
        }
    }
}

fn validate_legacy_receive_block(mut options: ValidateLegacyReceiveBlockOptions) -> ValidateOutput {
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

    let mut receive = BlockBuilder::legacy_receive()
        .previous(previous.hash())
        .source(BlockHash::from(42))
        .sign(&keypair);

    if let Some(setup) = &options.setup_block {
        receive = setup(receive);
    }
    let receive = receive.build();
    let old_account_info = create_account_info(&previous);

    let mut validator = BlockValidator {
        block: &receive,
        epochs: &Epochs::new(),
        work: &WORK_THRESHOLDS_STUB,
        block_exists: false,
        account: keypair.public_key(),
        frontier_missing: false,
        previous_block: Some(previous),
        old_account_info: Some(old_account_info.clone()),
        pending_receive_info: Some(PendingInfo {
            source: Account::from(7),
            amount: Amount::raw(10),
            epoch: Epoch::Epoch0,
        }),
        any_pending_exists: true,
        source_block_exists: true,
        seconds_since_epoch: 123456,
    };
    if let Some(setup) = &mut options.setup_validator {
        setup(&mut validator);
    }

    let result = validator.validate();
    ValidateOutput {
        seconds_since_epoch: validator.seconds_since_epoch,
        result,
        block: receive,
        old_account_info,
        account: keypair.public_key(),
    }
}
