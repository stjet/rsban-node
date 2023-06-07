use rsnano_core::{
    Account, AccountInfo, Amount, BlockBuilder, BlockDetails, BlockHash, BlockSideband,
    ConfirmationHeightInfo, Epoch, PendingInfo, PendingKey,
};

use crate::{
    block_rollback::rollback_planner::{RollbackPlanner, RollbackStep},
    ledger_constants::LEDGER_CONSTANTS_STUB,
    DEV_GENESIS_KEY,
};

#[test]
fn rollback_epoch() {
    let mut epoch_block = BlockBuilder::state()
        .link(*LEDGER_CONSTANTS_STUB.epochs.link(Epoch::Epoch1).unwrap())
        .sign(&DEV_GENESIS_KEY)
        .build();
    epoch_block.set_sideband(BlockSideband {
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
        head_block: &epoch_block,
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

#[test]
fn rollback_receive_block_which_performed_epoch_upgrade_undoes_epoch_upgrade() {
    let mut previous_block = BlockBuilder::state().balance(Amount::raw(100)).build();
    previous_block.set_sideband(BlockSideband {
        details: BlockDetails::new(Epoch::Epoch0, false, false, false),
        ..BlockSideband::create_test_instance()
    });

    let send_hash = BlockHash::from(123);

    let mut receive_block = BlockBuilder::state()
        .link(send_hash)
        .balance(Amount::raw(110))
        .build();
    receive_block.set_sideband(BlockSideband {
        details: BlockDetails::new(Epoch::Epoch1, false, false, true),
        source_epoch: Epoch::Epoch1,
        ..BlockSideband::create_test_instance()
    });

    let planner = RollbackPlanner {
        epochs: &LEDGER_CONSTANTS_STUB.epochs,
        head_block: &receive_block,
        account: receive_block.account(),
        current_account_info: AccountInfo {
            head: receive_block.hash(),
            representative: receive_block.representative().unwrap(),
            open_block: BlockHash::from(6),
            balance: receive_block.balance(),
            modified: 0,
            block_count: 2,
            epoch: Epoch::Epoch1,
        },
        previous_representative: Some(previous_block.representative().unwrap()),
        previous: Some(previous_block),
        linked_account: Account::from(456),
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
    assert_eq!(
        instructions.add_pending,
        Some((
            PendingKey::new(receive_block.account(), send_hash),
            PendingInfo {
                source: planner.linked_account,
                amount: Amount::raw(10),
                epoch: Epoch::Epoch1
            }
        ))
    );
}
