use rsnano_core::{work::WORK_THRESHOLDS_STUB, BlockBuilder, Epoch};

use crate::{
    block_insertion::BlockValidator, ledger_constants::LEDGER_CONSTANTS_STUB, DEV_GENESIS_KEY,
};

use super::create_account_info;

#[test]
#[ignore = "wip"]
fn updgrade_to_epoch_v1() {
    let open = BlockBuilder::legacy_open().with_sideband().build();

    let epoch = BlockBuilder::state()
        .account(open.account())
        .balance(open.balance())
        .representative(open.representative().unwrap())
        .link(*LEDGER_CONSTANTS_STUB.epochs.link(Epoch::Epoch1).unwrap())
        .previous(open.hash())
        .sign(&DEV_GENESIS_KEY)
        .build();

    let validator = BlockValidator {
        block: &epoch,
        epochs: &LEDGER_CONSTANTS_STUB.epochs,
        work: &WORK_THRESHOLDS_STUB,
        block_exists: false,
        account: open.account(),
        frontier_missing: false,
        old_account_info: Some(create_account_info(&open)),
        previous_block: Some(open),
        pending_receive_info: None,
        any_pending_exists: false,
        source_block_exists: false,
        seconds_since_epoch: 123456,
    };

    let result = validator.validate().unwrap();

    assert_eq!(result.set_account_info.epoch, Epoch::Epoch1);
}
