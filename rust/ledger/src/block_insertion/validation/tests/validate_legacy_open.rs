use rsnano_core::{
    work::WORK_THRESHOLDS_STUB, Account, AccountInfo, Amount, BlockBuilder, BlockDetails,
    BlockHash, BlockSideband, Epoch, Epochs, KeyPair, LegacyOpenBlockBuilder, PendingInfo,
    PendingKey,
};

use super::ValidateOutput;
use crate::{block_insertion::BlockValidator, ProcessResult};

#[test]
fn valid_legacy_open_block() {
    let output = validate_legacy_open_block(Default::default());

    let result = output.result.unwrap();
    assert_eq!(result.account, output.account);
    assert_eq!(
        result.set_sideband,
        BlockSideband {
            height: 1,
            timestamp: output.seconds_since_epoch,
            successor: BlockHash::zero(),
            account: output.account,
            balance: Amount::raw(100),
            details: BlockDetails::new(Epoch::Epoch0, false, true, false),
            source_epoch: Epoch::Epoch0
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
    assert_eq!(
        result.set_account_info,
        AccountInfo {
            head: output.block.hash(),
            representative: output.block.representative().unwrap(),
            open_block: output.block.hash(),
            balance: Amount::raw(100),
            modified: output.seconds_since_epoch,
            block_count: 1,
            epoch: Epoch::Epoch0
        }
    );
}

#[test]
fn fail_fork() {
    let output = validate_legacy_open_block(ValidateLegacyOpenBlockOptions {
        setup_validator: Some(&mut |validator| {
            validator.old_account_info = Some(AccountInfo::create_test_instance());
            validator.pending_receive_info = None;
        }),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::Fork))
}

#[test]
fn fail_if_duplicate() {
    let output = validate_legacy_open_block(ValidateLegacyOpenBlockOptions {
        setup_validator: Some(&mut |validator| {
            validator.old_account_info = Some(AccountInfo::create_test_instance());
            validator.pending_receive_info = None;
            validator.block_exists = true;
        }),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::Old))
}

#[test]
fn fail_with_gap_source_if_source_not_found() {
    let output = validate_legacy_open_block(ValidateLegacyOpenBlockOptions {
        setup_validator: Some(&mut |validator| {
            validator.pending_receive_info = None;
            validator.source_block_exists = false;
        }),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::GapSource))
}

#[test]
fn fail_if_signature_is_bad() {
    let output = validate_legacy_open_block(ValidateLegacyOpenBlockOptions {
        setup_block: Some(&|block| block.sign(&KeyPair::new())),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::BadSignature))
}

struct ValidateLegacyOpenBlockOptions<'a> {
    setup_block: Option<&'a dyn Fn(LegacyOpenBlockBuilder) -> LegacyOpenBlockBuilder>,
    setup_validator: Option<&'a mut dyn FnMut(&mut BlockValidator)>,
}

impl<'a> Default for ValidateLegacyOpenBlockOptions<'a> {
    fn default() -> Self {
        Self {
            setup_block: None,
            setup_validator: None,
        }
    }
}

fn validate_legacy_open_block(mut options: ValidateLegacyOpenBlockOptions) -> ValidateOutput {
    let keypair = KeyPair::new();

    let mut open = BlockBuilder::legacy_open()
        .account(Account::from(42))
        .source(BlockHash::from(500))
        .sign(&keypair);

    if let Some(setup) = &options.setup_block {
        open = setup(open);
    }
    let send = open.build();

    let mut validator = BlockValidator {
        block: &send,
        epochs: &Epochs::new(),
        work: &WORK_THRESHOLDS_STUB,
        block_exists: false,
        account: keypair.public_key(),
        frontier_missing: false,
        previous_block: None,
        old_account_info: None,
        pending_receive_info: Some(PendingInfo {
            source: Account::from(7),
            amount: Amount::raw(100),
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
        block: send,
        old_account_info: Default::default(),
        account: keypair.public_key(),
    }
}
