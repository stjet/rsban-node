use rsnano_core::{
    Account, Amount, BlockHash, ConfirmationHeightInfo, Epoch, PendingInfo, PendingKey,
    TestAccountChain,
};

use crate::{
    block_rollback::rollback_planner::{RollbackPlanner, RollbackStep},
    ledger_constants::LEDGER_CONSTANTS_STUB,
};

#[test]
fn rollback_epoch1() {
    let mut chain = TestAccountChain::new_opened_chain();
    chain.add_epoch_v1();

    let planner = RollbackPlanner {
        epochs: &LEDGER_CONSTANTS_STUB.epochs,
        head_block: chain.latest_block(),
        account: chain.account(),
        current_account_info: chain.account_info(),
        previous_representative: None,
        previous: Some(chain.block(chain.height() - 1).clone()),
        linked_account: Account::zero(),
        pending_receive: None,
        latest_block_for_destination: None,
        confirmation_height: ConfirmationHeightInfo {
            height: 0,
            frontier: BlockHash::zero(),
        },
    };

    let result = planner
        .roll_back_head_block()
        .expect("rollback should succeed");
    let RollbackStep::RollBackBlock(instructions) = result else { panic!("expected RollBackBlock") };
    assert_eq!(instructions.set_account_info.epoch, Epoch::Epoch0);
}

#[test]
fn rollback_receive_block_which_performed_epoch1_upgrade_undoes_epoch_upgrade() {
    let mut chain = TestAccountChain::new_opened_chain();
    let receive_block = chain.new_receive_block().amount_received(1).build();
    let receive_block = chain.add_block(receive_block, Epoch::Epoch1).clone();
    let previous_block = chain.block(chain.height() - 1).clone();

    let planner = RollbackPlanner {
        epochs: &LEDGER_CONSTANTS_STUB.epochs,
        head_block: &receive_block,
        account: receive_block.account(),
        current_account_info: chain.account_info(),
        previous_representative: Some(previous_block.representative().unwrap()),
        previous: Some(previous_block.clone()),
        linked_account: Account::from(456),
        pending_receive: None,
        latest_block_for_destination: None,
        confirmation_height: ConfirmationHeightInfo {
            height: 0,
            frontier: BlockHash::from(0),
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
            PendingKey::new(receive_block.account(), receive_block.link().into()),
            PendingInfo {
                source: planner.linked_account,
                amount: Amount::raw(1),
                epoch: Epoch::Epoch1
            }
        ))
    );
}

#[test]
fn rollback_epoch_v2() {
    let mut chain = TestAccountChain::new_opened_chain();
    let previous_block = chain.add_epoch_v1().clone();
    let epoch2_block = chain.add_epoch_v2().clone();

    let planner = RollbackPlanner {
        epochs: &LEDGER_CONSTANTS_STUB.epochs,
        head_block: &epoch2_block,
        account: epoch2_block.account_calculated(),
        current_account_info: chain.account_info(),
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
    assert_eq!(instructions.set_account_info.epoch, Epoch::Epoch1);
}
