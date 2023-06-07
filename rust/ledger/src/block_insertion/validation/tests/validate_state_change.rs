use rsnano_core::{
    work::WORK_THRESHOLDS_STUB, AccountInfo, BlockBuilder, BlockDetails, BlockHash, BlockSideband,
    Epoch, Epochs, KeyPair,
};

use crate::block_insertion::BlockValidator;

use super::{create_test_account_info, ValidateOutput, ValidateStateBlockOptions};

#[test]
fn valid_change_block() {
    let output = validate_change_block(Default::default());
    let change_block = &output.block;
    let old_account_info = &output.old_account_info;
    let result = output.result.unwrap();

    assert_eq!(
        result.set_account_info,
        AccountInfo {
            head: change_block.hash(),
            representative: change_block.representative().unwrap(),
            open_block: old_account_info.open_block,
            balance: old_account_info.balance,
            modified: output.seconds_since_epoch,
            block_count: old_account_info.block_count + 1,
            epoch: old_account_info.epoch,
        }
    );
    assert_eq!(result.delete_pending, None, "delete pending");
    assert_eq!(result.insert_pending, None, "insert pending");

    assert_eq!(
        result.set_sideband,
        BlockSideband {
            height: old_account_info.block_count + 1,
            timestamp: output.seconds_since_epoch,
            successor: BlockHash::zero(),
            account: change_block.account(),
            balance: old_account_info.balance,
            details: BlockDetails::new(old_account_info.epoch, false, false, false),
            source_epoch: Epoch::Epoch0
        },
        "sideband"
    );
    assert_eq!(result.is_epoch_block, false);
}

fn validate_change_block(mut options: ValidateStateBlockOptions) -> ValidateOutput {
    let keypair = KeyPair::new();

    let previous = BlockBuilder::state()
        .account(keypair.public_key())
        .balance(1000)
        .build();

    let mut change = BlockBuilder::state()
        .account(keypair.public_key())
        .previous(previous.hash())
        .balance(previous.balance())
        .representative(12345678)
        .link(0)
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
        account: change.account(),
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
