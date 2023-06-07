use rsnano_core::{
    work::WORK_THRESHOLDS_STUB, AccountInfo, Amount, BlockBuilder, BlockDetails, BlockHash,
    BlockSideband, Epoch, Epochs, KeyPair, PendingInfo, PendingKey,
};

use crate::block_insertion::{validation::tests::ValidateStateBlockOptions, BlockValidator};

use super::{create_test_account_info, ValidateOutput};

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
    let old_account_info = create_test_account_info(&previous);

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
        account: keypair.public_key(),
    }
}
