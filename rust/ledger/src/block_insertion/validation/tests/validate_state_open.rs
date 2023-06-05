use rsnano_core::{
    work::WORK_THRESHOLDS_STUB, Account, AccountInfo, Amount, BlockBuilder, BlockDetails,
    BlockHash, BlockSideband, Epoch, Epochs, KeyPair, PendingInfo, PendingKey,
};

use crate::{
    block_insertion::{validation::tests::ValidateStateBlockOptions, BlockValidator},
    ProcessResult,
};

use super::ValidateOutput;

#[test]
fn valid_open_block() {
    let output = validate_open_block(Default::default());
    let open_block = &output.block;
    let result = output.result.unwrap();
    assert_eq!(
        result.set_sideband,
        BlockSideband {
            height: 1,
            timestamp: output.seconds_since_epoch,
            successor: BlockHash::zero(),
            account: open_block.account(),
            balance: open_block.balance(),
            details: BlockDetails::new(Epoch::Epoch2, false, true, false),
            source_epoch: Epoch::Epoch2
        }
    );
    assert_eq!(
        result.delete_pending,
        Some(PendingKey::for_receive_block(open_block))
    );

    assert_eq!(
        result.set_account_info,
        AccountInfo {
            head: open_block.hash(),
            representative: open_block.representative().unwrap(),
            open_block: open_block.hash(),
            balance: open_block.balance(),
            modified: output.seconds_since_epoch,
            block_count: 1,
            epoch: Epoch::Epoch2
        }
    )
}

#[test]
fn fails_with_fork_if_account_already_opened() {
    let output = validate_open_block(ValidateStateBlockOptions {
        setup_validator: Some(&mut |validator| {
            validator.old_account_info = Some(AccountInfo::create_test_instance());
            validator.pending_receive_info = None;
        }),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::Fork));
}

#[test]
fn fails_with_gap_previous_if_open_block_has_previous_block() {
    let output = validate_open_block(ValidateStateBlockOptions {
        setup_block: Some(&|block| block.previous(123)),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::GapPrevious));
}

#[test]
fn fails_with_gap_source_if_link_missing() {
    let output = validate_open_block(ValidateStateBlockOptions {
        setup_block: Some(&|block| block.link(0)),
        ..Default::default()
    });
    assert_eq!(output.result, Err(ProcessResult::GapSource));
}

fn validate_open_block(mut options: ValidateStateBlockOptions) -> ValidateOutput {
    let keypair = KeyPair::new();
    let send_hash = BlockHash::from(12345);
    let mut open = BlockBuilder::state()
        .account(keypair.public_key())
        .previous(0)
        .link(send_hash)
        .balance(500)
        .sign(&keypair);
    if let Some(setup) = &options.setup_block {
        open = setup(open);
    }
    let open = open.build();

    let mut validator = BlockValidator {
        block: &open,
        epochs: &Epochs::new(),
        work: &WORK_THRESHOLDS_STUB,
        block_exists: false,
        account: open.account(),
        frontier_missing: false,
        previous_block: None,
        old_account_info: None,
        pending_receive_info: Some(PendingInfo {
            source: Account::from(42),
            amount: Amount::raw(500),
            epoch: Epoch::Epoch2,
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
        block: open,
        old_account_info: Default::default(),
        account: keypair.public_key(),
    }
}
