use crate::block_rollback::tests::RollbackTest;
use rsnano_core::{Account, Amount, Epoch, PendingInfo, PendingKey, TestAccountChain};

#[test]
fn rollback_epoch1() {
    let mut chain = TestAccountChain::new_opened_chain();
    chain.add_epoch_v1();

    let instructions = RollbackTest::for_chain(&chain).assert_rollback_succeeds();

    assert_eq!(instructions.set_account_info.epoch, Epoch::Epoch0);
}

#[test]
fn rollback_receive_block_which_performed_epoch1_upgrade_undoes_epoch_upgrade() {
    let mut chain = TestAccountChain::new_opened_chain();
    let receive_block = chain.new_receive_block().amount_received(1).build();
    let receive_block = chain.add_block(receive_block, Epoch::Epoch1).clone();

    let instructions = RollbackTest::for_chain(&chain)
        .with_linked_account(456)
        .assert_rollback_succeeds();

    assert_eq!(instructions.set_account_info.epoch, Epoch::Epoch0);
    assert_eq!(
        instructions.add_pending,
        Some((
            PendingKey::new(receive_block.account(), receive_block.link().into()),
            PendingInfo {
                source: Account::from(456),
                amount: Amount::raw(1),
                epoch: Epoch::Epoch1
            }
        ))
    );
}

#[test]
fn rollback_epoch_v2() {
    let mut chain = TestAccountChain::new_opened_chain();
    chain.add_epoch_v1();
    chain.add_epoch_v2();

    let instructions = RollbackTest::for_chain(&chain).assert_rollback_succeeds();

    assert_eq!(instructions.set_account_info.epoch, Epoch::Epoch1);
}
