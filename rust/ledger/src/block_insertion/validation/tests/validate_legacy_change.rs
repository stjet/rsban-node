use super::ValidateOutput;
use crate::{
    block_insertion::BlockValidator, test_helpers::create_test_account_info, ProcessResult,
};
use rsnano_core::{
    work::WORK_THRESHOLDS_STUB, Account, BlockBuilder, BlockDetails, BlockHash, BlockSideband,
    Epoch, Epochs, KeyPair, LegacyChangeBlockBuilder,
};

// Legacy Change
//------------------------------------------------------------------------------

#[test]
fn valid_legacy_change_block() {
    let output = validate_legacy_change_block(Default::default());

    let result = output.result.unwrap();
    assert_eq!(result.account, output.account);
    assert_eq!(
        result.set_sideband,
        BlockSideband {
            height: output.old_account_info.block_count + 1,
            timestamp: output.seconds_since_epoch,
            successor: BlockHash::zero(),
            account: output.account,
            balance: output.old_account_info.balance,
            details: BlockDetails::new(Epoch::Epoch0, false, false, false),
            source_epoch: Epoch::Epoch0,
        }
    );
    assert_eq!(
        result.set_account_info.representative,
        output.block.representative().unwrap()
    );
    assert_eq!(
        result.set_account_info.balance,
        output.old_account_info.balance
    );
}

#[test]
fn allow_changing_representative_to_zero() {
    let output = validate_legacy_change_block(ValidateLegacyChangeBlockOptions {
        setup_block: Some(&|block| block.representative(Account::zero())),
        ..Default::default()
    });

    let result = output.result.unwrap();
    assert_eq!(result.set_account_info.representative, Account::zero());
}

#[test]
fn allow_changing_from_zero_rep_to_real_rep() {
    let output = validate_legacy_change_block(ValidateLegacyChangeBlockOptions {
        setup_validator: Some(&mut |validator| {
            validator.old_account_info.as_mut().unwrap().representative = Account::zero()
        }),
        setup_block: Some(&|block| block.representative(Account::from(42))),
    });

    let result = output.result.unwrap();
    assert_eq!(result.set_account_info.representative, Account::from(42));
}

#[test]
fn fails_with_block_position_if_legacy_change_follows_state_block() {
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

    let legacy_change = BlockBuilder::legacy_change()
        .previous(previous.hash())
        .representative(Account::from(123))
        .sign(&keypair)
        .build();
    let old_account_info = Some(create_test_account_info(&previous));
    let validator = BlockValidator {
        block: &legacy_change,
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

struct ValidateLegacyChangeBlockOptions<'a> {
    setup_block: Option<&'a dyn Fn(LegacyChangeBlockBuilder) -> LegacyChangeBlockBuilder>,
    setup_validator: Option<&'a mut dyn FnMut(&mut BlockValidator)>,
}

impl<'a> Default for ValidateLegacyChangeBlockOptions<'a> {
    fn default() -> Self {
        Self {
            setup_block: None,
            setup_validator: None,
        }
    }
}

fn validate_legacy_change_block(mut options: ValidateLegacyChangeBlockOptions) -> ValidateOutput {
    let keypair = KeyPair::new();

    let mut previous = BlockBuilder::legacy_open()
        .account(keypair.public_key())
        .with_sideband()
        .build();
    previous.set_sideband(BlockSideband {
        successor: BlockHash::zero(),
        account: keypair.public_key(),
        details: BlockDetails::new(Epoch::Epoch0, false, true, false),
        ..BlockSideband::create_test_instance()
    });

    let mut change = BlockBuilder::legacy_change()
        .previous(previous.hash())
        .representative(Account::from(123456))
        .sign(&keypair);

    if let Some(setup) = &options.setup_block {
        change = setup(change);
    }
    let change = change.build();
    let old_account_info = create_test_account_info(&previous);

    let mut validator = BlockValidator {
        block: &change,
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
        block: change,
        old_account_info,
        account: keypair.public_key(),
    }
}
