use rsnano_core::{
    Account, AccountInfo, Amount, BlockBuilder, BlockDetails, BlockHash, BlockSideband,
    ConfirmationHeightInfo, Epoch,
};

use crate::{
    block_rollback::rollback_planner::{RollbackPlanner, RollbackStep},
    ledger_constants::LEDGER_CONSTANTS_STUB,
};

#[test]
fn rollback_epoch() {
    let mut head_block = BlockBuilder::state().build();
    head_block.set_sideband(BlockSideband {
        details: BlockDetails::new(Epoch::Epoch1, false, false, true),
        ..BlockSideband::create_test_instance()
    });

    let mut previous_block = BlockBuilder::state().build();
    previous_block.set_sideband(BlockSideband {
        details: BlockDetails::new(Epoch::Epoch0, false, false, false),
        ..BlockSideband::create_test_instance()
    });

    let planner = RollbackPlanner {
        epochs: &LEDGER_CONSTANTS_STUB.epochs,
        head_block: &head_block,
        account: Account::from(2),
        current_account_info: AccountInfo {
            head: BlockHash::from(1),
            representative: Account::from(5),
            open_block: BlockHash::from(6),
            balance: Amount::raw(100),
            modified: 0,
            block_count: 2,
            epoch: Epoch::Epoch1,
        },
        previous_representative: None,
        previous: Some(previous_block),
        linked_account: Account::zero(),
        pending_receive: None,
        latest_block_for_destination: None,
        confirmation_height: ConfirmationHeightInfo {
            height: 2,
            frontier: BlockHash::from(1),
        },
    };

    let result = planner
        .roll_back_head_block()
        .expect("rollback should succeed");
    let RollbackStep::RollBackBlock(instructions) = result else { panic!("expected RollBackBlock") };
    assert_eq!(instructions.set_account_info.epoch, Epoch::Epoch0);
}
