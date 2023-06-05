use rsnano_core::{
    work::WORK_THRESHOLDS_STUB, Account, AccountInfo, Amount, BlockBuilder, BlockDetails,
    BlockEnum, BlockHash, BlockSideband, Epoch, Epochs, KeyPair, PendingInfo, PendingKey,
    StateBlockBuilder,
};

use crate::{
    block_insertion::{BlockInsertInstructions, BlockValidator},
    ProcessResult,
};

// State Send
//------------------------------------------------------------------------------

#[test]
fn valid_send_block() {
    let output = validate_send_block(Default::default());
    let send_block = &output.block;
    let old_account_info = &output.old_account_info;
    let result = output.result.unwrap();

    assert_eq!(result.account, send_block.account(), "account");
    assert_eq!(
        result.set_account_info,
        AccountInfo {
            head: send_block.hash(),
            representative: send_block.representative().unwrap(),
            open_block: old_account_info.open_block,
            balance: send_block.balance(),
            modified: output.seconds_since_epoch,
            block_count: old_account_info.block_count + 1,
            epoch: old_account_info.epoch,
        }
    );
    assert_eq!(result.delete_pending, None, "delete pending");
    assert_eq!(
        result.insert_pending,
        Some((
            PendingKey::for_send_block(&send_block),
            PendingInfo {
                amount: Amount::raw(100),
                epoch: old_account_info.epoch,
                source: send_block.account()
            }
        )),
        "insert pending"
    );

    assert_eq!(
        result.set_sideband,
        BlockSideband {
            height: old_account_info.block_count + 1,
            timestamp: output.seconds_since_epoch,
            successor: BlockHash::zero(),
            account: send_block.account(),
            balance: Amount::raw(900),
            details: BlockDetails::new(old_account_info.epoch, true, false, false),
            source_epoch: Epoch::Epoch0
        },
        "sideband"
    );
    assert_eq!(result.is_epoch_block, false);
}

#[test]
fn sends_to_burn_account_are_valid() {
    let output = validate_send_block(ValidateStateBlockOptions {
        setup_block: Some(&|block| block.link(0)),
        ..Default::default()
    });

    assert!(output.result.is_ok());
}

// State Receive
//------------------------------------------------------------------------------

#[test]
fn valid_receive_block() {
    let output = validate_receive_block(Default::default());
    let receive = &output.block;
    let old_account_info = &output.old_account_info;
    let result = output.result.unwrap();

    assert_eq!(result.account, receive.account(), "account");
    assert_eq!(
        result.set_account_info,
        AccountInfo {
            head: receive.hash(),
            representative: receive.representative().unwrap(),
            open_block: old_account_info.open_block,
            balance: receive.balance(),
            modified: output.seconds_since_epoch,
            block_count: old_account_info.block_count + 1,
            epoch: old_account_info.epoch,
        }
    );
    assert_eq!(
        result.set_sideband,
        BlockSideband {
            height: old_account_info.block_count + 1,
            timestamp: output.seconds_since_epoch,
            successor: BlockHash::zero(),
            account: receive.account(),
            balance: Amount::raw(1200),
            details: BlockDetails::new(old_account_info.epoch, false, true, false),
            source_epoch: Epoch::Epoch0
        },
        "sideband"
    );
    assert_eq!(
        result.delete_pending,
        Some(PendingKey::new(receive.account(), receive.link().into()))
    );
    assert_eq!(result.insert_pending, None);
}
#[test]
fn fails_with_gap_source_if_send_block_not_found() {
    let output = validate_receive_block(ValidateStateBlockOptions {
        setup_validator: Some(&mut |validator| {
            validator.source_block_exists = false;
            validator.pending_receive_info = None;
        }),
        ..Default::default()
    });

    assert_eq!(output.result, Err(ProcessResult::GapSource));
}

#[test]
fn fails_with_balance_mismatch_if_amount_is_wrong() {
    let output = validate_receive_block(ValidateStateBlockOptions {
        setup_block: Some(&mut |block| block.balance(9999999)),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::BalanceMismatch));
}

#[test]
fn fails_with_balance_mismatch_if_no_link_provided() {
    let output = validate_receive_block(ValidateStateBlockOptions {
        setup_block: Some(&mut |block| block.link(0)),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::BalanceMismatch));
}

#[test]
fn fails_with_unreceivable_if_receiving_from_wrong_account() {
    let output = validate_receive_block(ValidateStateBlockOptions {
        setup_validator: Some(&mut |validator| validator.pending_receive_info = None),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::Unreceivable));
}

// Helpers
//------------------------------------------------------------------------------

struct ValidateStateBlockOptions<'a> {
    setup_block: Option<&'a dyn Fn(StateBlockBuilder) -> StateBlockBuilder>,
    setup_validator: Option<&'a mut dyn FnMut(&mut BlockValidator)>,
}

impl<'a> Default for ValidateStateBlockOptions<'a> {
    fn default() -> Self {
        Self {
            setup_block: None,
            setup_validator: None,
        }
    }
}

struct ValidateOutput {
    block: BlockEnum,
    result: Result<BlockInsertInstructions, ProcessResult>,
    old_account_info: AccountInfo,
    seconds_since_epoch: u64,
}

fn validate_send_block(mut options: ValidateStateBlockOptions) -> ValidateOutput {
    let keypair = KeyPair::new();

    let previous = BlockBuilder::state()
        .account(keypair.public_key())
        .balance(1000)
        .build();

    let mut send = BlockBuilder::state()
        .account(keypair.public_key())
        .previous(previous.hash())
        .balance(900)
        .sign(&keypair);
    if let Some(setup) = &options.setup_block {
        send = setup(send);
    }
    let send_block = send.build();
    let old_account_info = create_account_info(&previous);

    let mut validator = BlockValidator {
        block: &send_block,
        epochs: &Epochs::new(),
        work: &WORK_THRESHOLDS_STUB,
        block_exists: false,
        account: send_block.account(),
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
        block: send_block,
        old_account_info,
    }
}

fn validate_receive_block(mut options: ValidateStateBlockOptions) -> ValidateOutput {
    let keypair = KeyPair::new();

    let previous = BlockBuilder::state()
        .account(keypair.public_key())
        .balance(1000)
        .build();

    let mut receive = BlockBuilder::state()
        .account(keypair.public_key())
        .previous(previous.hash())
        .link(BlockHash::from(12345))
        .balance(1200)
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
        account: receive.account(),
        frontier_missing: false,
        previous_block: Some(previous),
        old_account_info: Some(old_account_info.clone()),
        pending_receive_info: Some(PendingInfo {
            source: Account::from(12345),
            amount: Amount::raw(200),
            epoch: Epoch::Epoch0,
        }),
        any_pending_exists: false,
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
    }
}

fn create_account_info(block: &BlockEnum) -> AccountInfo {
    AccountInfo {
        balance: block.balance(),
        head: block.hash(),
        ..AccountInfo::create_test_instance()
    }
}
