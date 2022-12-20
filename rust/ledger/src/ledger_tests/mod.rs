use std::ops::Deref;
use std::sync::atomic::Ordering;

mod helpers;
use crate::{
    ledger_constants::LEDGER_CONSTANTS_STUB, Ledger, LedgerCache, UncementedInfo, DEV_GENESIS,
    DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_KEY,
};
pub(crate) use helpers::*;
use rsnano_core::{
    Account, Amount, BlockBuilder, BlockHash, KeyPair, QualifiedRoot, Root, GXRB_RATIO,
};

mod empty_ledger;
mod epoch_v1;
mod epoch_v2;
mod process_change;
mod process_legacy_change;
mod process_legacy_open;
mod process_legacy_receive;
mod process_legacy_send;
mod process_open;
mod process_receive;
mod process_send;
mod pruning;
mod rollback_legacy_change;
mod rollback_legacy_open;
mod rollback_legacy_receive;
mod rollback_legacy_send;
mod rollback_state;

#[test]
fn ledger_successor() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger.successor(
            txn.txn(),
            &QualifiedRoot::new(Root::zero(), *DEV_GENESIS_HASH)
        ),
        Some(send.send_block)
    );
}

#[test]
fn ledger_successor_genesis() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    setup_legacy_send_block(&ctx, txn.as_mut());
    let genesis = DEV_GENESIS.read().unwrap().clone();

    assert_eq!(
        ctx.ledger.successor(
            txn.txn(),
            &QualifiedRoot::new(DEV_GENESIS_ACCOUNT.deref().into(), BlockHash::zero())
        ),
        Some(genesis)
    );
}

#[test]
fn latest_root_empty() {
    let ctx = LedgerContext::empty();
    let txn = ctx.ledger.read_txn();
    assert_eq!(
        ctx.ledger.latest_root(txn.txn(), &Account::from(1)),
        Root::from(1)
    );
}

#[test]
fn latest_root() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger.latest_root(txn.txn(), &DEV_GENESIS_ACCOUNT),
        send.send_block.hash().into()
    );
}

#[test]
fn send_open_receive_vote_weight() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let receiver = ctx.block_factory();

    let mut send1 = genesis
        .legacy_send(txn.txn())
        .destination(receiver.account())
        .amount(Amount::new(50))
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let mut send2 = genesis
        .legacy_send(txn.txn())
        .destination(receiver.account())
        .amount(Amount::new(50))
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    let mut open = receiver.legacy_open(send1.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

    let mut receive = receiver.legacy_receive(txn.txn(), send2.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

    assert_eq!(ctx.ledger.weight(&receiver.account()), Amount::new(100));
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        LEDGER_CONSTANTS_STUB.genesis_amount - Amount::new(100)
    );
}

#[test]
fn send_open_receive_rollback() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let receiver = AccountBlockFactory::new(&ctx.ledger);

    let mut send1 = genesis
        .legacy_send(txn.txn())
        .destination(receiver.account())
        .amount(Amount::new(50))
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let mut send2 = genesis
        .legacy_send(txn.txn())
        .destination(receiver.account())
        .amount(Amount::new(50))
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    let mut open = receiver.legacy_open(send1.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

    let mut receive = receiver.legacy_receive(txn.txn(), send2.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();
    let rep_account = Account::from(1);

    let mut change = genesis
        .legacy_change(txn.txn())
        .representative(rep_account)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut change).unwrap();

    ctx.ledger.rollback(txn.as_mut(), &receive.hash()).unwrap();

    assert_eq!(ctx.ledger.weight(&receiver.account()), Amount::new(50));
    assert_eq!(ctx.ledger.weight(&DEV_GENESIS_ACCOUNT), Amount::zero());
    assert_eq!(
        ctx.ledger.weight(&rep_account),
        LEDGER_CONSTANTS_STUB.genesis_amount - Amount::new(100)
    );

    ctx.ledger.rollback(txn.as_mut(), &open.hash()).unwrap();

    assert_eq!(ctx.ledger.weight(&receiver.account()), Amount::zero());
    assert_eq!(ctx.ledger.weight(&DEV_GENESIS_ACCOUNT), Amount::zero());
    assert_eq!(
        ctx.ledger.weight(&rep_account),
        LEDGER_CONSTANTS_STUB.genesis_amount - Amount::new(100)
    );

    ctx.ledger.rollback(txn.as_mut(), &change.hash()).unwrap();

    assert_eq!(ctx.ledger.weight(&receiver.account()), Amount::zero());
    assert_eq!(ctx.ledger.weight(&rep_account), Amount::zero());
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        LEDGER_CONSTANTS_STUB.genesis_amount - Amount::new(100)
    );

    ctx.ledger.rollback(txn.as_mut(), &send2.hash()).unwrap();

    assert_eq!(ctx.ledger.weight(&receiver.account()), Amount::zero());
    assert_eq!(ctx.ledger.weight(&rep_account), Amount::zero());
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        LEDGER_CONSTANTS_STUB.genesis_amount - Amount::new(50)
    );

    ctx.ledger.rollback(txn.as_mut(), &send1.hash()).unwrap();

    assert_eq!(ctx.ledger.weight(&receiver.account()), Amount::zero());
    assert_eq!(ctx.ledger.weight(&rep_account), Amount::zero());
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        LEDGER_CONSTANTS_STUB.genesis_amount
    );
}

#[test]
fn bootstrap_rep_weight() {
    let ctx = LedgerContext::empty();
    ctx.ledger.set_bootstrap_weight_max_blocks(3);
    let genesis = ctx.genesis_block_factory();
    let representative_key = KeyPair::new();
    let representative_account = representative_key.public_key().into();
    {
        let mut txn = ctx.ledger.rw_txn();
        let mut send = genesis
            .legacy_send(txn.txn())
            .destination(representative_account)
            .amount(Amount::new(50))
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send).unwrap();
    }
    {
        let mut weights = ctx.ledger.bootstrap_weights.lock().unwrap();
        weights.insert(representative_account, Amount::new(1000));
    }
    assert_eq!(ctx.ledger.cache.block_count.load(Ordering::Relaxed), 2);
    assert_eq!(
        ctx.ledger.weight(&representative_account),
        Amount::new(1000)
    );
    {
        let mut txn = ctx.ledger.rw_txn();
        let mut send = genesis
            .legacy_send(txn.txn())
            .destination(representative_account)
            .amount(Amount::new(50))
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send).unwrap();
    }
    assert_eq!(ctx.ledger.cache.block_count.load(Ordering::Relaxed), 3);
    assert_eq!(ctx.ledger.weight(&representative_account), Amount::zero());
}

#[test]
fn block_destination_source() {
    let ctx = LedgerContext::empty();
    let ledger = &ctx.ledger;
    let mut txn = ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let dest_account = Account::from(1000);

    let mut send_to_dest = genesis
        .legacy_send(txn.txn())
        .destination(dest_account)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send_to_dest).unwrap();

    let mut send_to_self = genesis
        .legacy_send(txn.txn())
        .destination(genesis.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send_to_self).unwrap();

    let mut receive = genesis
        .legacy_receive(txn.txn(), send_to_self.hash())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

    let mut send_to_dest_2 = genesis.send(txn.txn()).link(dest_account).build();
    ctx.ledger
        .process(txn.as_mut(), &mut send_to_dest_2)
        .unwrap();

    let mut send_to_self_2 = genesis.send(txn.txn()).link(genesis.account()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut send_to_self_2)
        .unwrap();

    let mut receive2 = genesis.receive(txn.txn(), send_to_self_2.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut receive2).unwrap();

    let block1 = send_to_dest;
    let block2 = send_to_self;
    let block3 = receive;
    let block4 = send_to_dest_2;
    let block5 = send_to_self_2;
    let block6 = receive2;

    assert_eq!(ledger.balance(txn.txn(), &block6.hash()), block6.balance());
    assert_eq!(ledger.block_destination(txn.txn(), &block1), dest_account);
    assert_eq!(ledger.block_source(txn.txn(), &block1), BlockHash::zero());

    assert_eq!(
        ledger.block_destination(txn.txn(), &block2),
        *DEV_GENESIS_ACCOUNT
    );
    assert_eq!(ledger.block_source(txn.txn(), &block2), BlockHash::zero());

    assert_eq!(
        ledger.block_destination(txn.txn(), &block3),
        Account::zero()
    );
    assert_eq!(ledger.block_source(txn.txn(), &block3), block2.hash());

    assert_eq!(ledger.block_destination(txn.txn(), &block4), dest_account);
    assert_eq!(ledger.block_source(txn.txn(), &block4), BlockHash::zero());

    assert_eq!(
        ledger.block_destination(txn.txn(), &block5),
        *DEV_GENESIS_ACCOUNT
    );
    assert_eq!(ledger.block_source(txn.txn(), &block5), BlockHash::zero());

    assert_eq!(
        ledger.block_destination(txn.txn(), &block6),
        Account::zero()
    );
    assert_eq!(ledger.block_source(txn.txn(), &block6), block5.hash());
}

#[test]
fn state_account() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let mut send = BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(*DEV_GENESIS_HASH)
        .balance(LEDGER_CONSTANTS_STUB.genesis_amount - Amount::new(*GXRB_RATIO))
        .link(*DEV_GENESIS_ACCOUNT)
        .sign(&DEV_GENESIS_KEY)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();
    assert_eq!(
        ctx.ledger.account(txn.txn(), &send.hash()),
        Some(*DEV_GENESIS_ACCOUNT)
    );
}

mod dependents_confirmed {
    use super::*;

    #[test]
    fn genesis_is_confirmed() {
        let ctx = LedgerContext::empty();
        let txn = ctx.ledger.read_txn();

        assert_eq!(
            ctx.ledger
                .dependents_confirmed(txn.txn(), &DEV_GENESIS.read().unwrap()),
            true
        );
    }

    #[test]
    fn send_dependents_are_confirmed_if_previous_block_is_confirmed() {
        let ctx = LedgerContext::empty();
        let mut txn = ctx.ledger.rw_txn();
        let destination = ctx.block_factory();

        let mut send = ctx
            .genesis_block_factory()
            .send(txn.txn())
            .link(destination.account())
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

        assert_eq!(ctx.ledger.dependents_confirmed(txn.txn(), &send), true);
    }

    #[test]
    fn send_dependents_are_unconfirmed_if_previous_block_is_unconfirmed() {
        let ctx = LedgerContext::empty();
        let mut txn = ctx.ledger.rw_txn();

        let mut send1 = ctx.genesis_block_factory().send(txn.txn()).build();
        ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

        let mut send2 = ctx.genesis_block_factory().send(txn.txn()).build();
        ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

        assert_eq!(ctx.ledger.dependents_confirmed(txn.txn(), &send2), false);
    }

    #[test]
    fn open_dependents_are_unconfirmed_if_send_block_is_unconfirmed() {
        let ctx = LedgerContext::empty();
        let mut txn = ctx.ledger.rw_txn();
        let destination = ctx.block_factory();

        let mut send = ctx
            .genesis_block_factory()
            .send(txn.txn())
            .link(destination.account())
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

        let mut open = destination.open(txn.txn(), send.hash()).build();
        ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

        assert_eq!(ctx.ledger.dependents_confirmed(txn.txn(), &open), false);
    }

    #[test]
    fn open_dependents_are_confirmed_if_send_block_is_confirmed() {
        let ctx = LedgerContext::empty();
        let mut txn = ctx.ledger.rw_txn();
        let destination = ctx.block_factory();

        let mut send = ctx
            .genesis_block_factory()
            .send(txn.txn())
            .link(destination.account())
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send).unwrap();
        ctx.inc_confirmation_height(txn.as_mut(), &DEV_GENESIS_ACCOUNT);

        let mut open = destination.open(txn.txn(), send.hash()).build();
        ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

        assert_eq!(ctx.ledger.dependents_confirmed(txn.txn(), &open), true);
    }

    #[test]
    fn receive_dependents_are_unconfirmed_if_send_block_is_unconfirmed() {
        let ctx = LedgerContext::empty();
        let mut txn = ctx.ledger.rw_txn();

        let destination = ctx.block_factory();

        let mut send1 = ctx
            .genesis_block_factory()
            .send(txn.txn())
            .link(destination.account())
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();
        ctx.inc_confirmation_height(txn.as_mut(), &DEV_GENESIS_ACCOUNT);

        let mut send2 = ctx
            .genesis_block_factory()
            .send(txn.txn())
            .link(destination.account())
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

        let mut open = destination.open(txn.txn(), send1.hash()).build();
        ctx.ledger.process(txn.as_mut(), &mut open).unwrap();
        ctx.inc_confirmation_height(txn.as_mut(), &destination.account());

        let mut receive = destination.receive(txn.txn(), send2.hash()).build();
        ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

        assert_eq!(ctx.ledger.dependents_confirmed(txn.txn(), &receive), false);
    }

    #[test]
    fn receive_dependents_are_unconfirmed_if_previous_block_is_unconfirmed() {
        let ctx = LedgerContext::empty();
        let mut txn = ctx.ledger.rw_txn();

        let destination = ctx.block_factory();

        let mut send1 = ctx
            .genesis_block_factory()
            .send(txn.txn())
            .link(destination.account())
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();
        ctx.inc_confirmation_height(txn.as_mut(), &DEV_GENESIS_ACCOUNT);

        let mut send2 = ctx
            .genesis_block_factory()
            .send(txn.txn())
            .link(destination.account())
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();
        ctx.inc_confirmation_height(txn.as_mut(), &DEV_GENESIS_ACCOUNT);

        let mut open = destination.open(txn.txn(), send1.hash()).build();
        ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

        let mut receive = destination.receive(txn.txn(), send2.hash()).build();
        ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

        assert_eq!(ctx.ledger.dependents_confirmed(txn.txn(), &receive), false);
    }

    #[test]
    fn receive_dependents_are_confirmed_if_previous_block_and_send_block_are_confirmed() {
        let ctx = LedgerContext::empty();
        let mut txn = ctx.ledger.rw_txn();

        let destination = ctx.block_factory();

        let mut send1 = ctx
            .genesis_block_factory()
            .send(txn.txn())
            .link(destination.account())
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();
        ctx.inc_confirmation_height(txn.as_mut(), &DEV_GENESIS_ACCOUNT);

        let mut send2 = ctx
            .genesis_block_factory()
            .send(txn.txn())
            .link(destination.account())
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();
        ctx.inc_confirmation_height(txn.as_mut(), &DEV_GENESIS_ACCOUNT);

        let mut open = destination.open(txn.txn(), send1.hash()).build();
        ctx.ledger.process(txn.as_mut(), &mut open).unwrap();
        ctx.inc_confirmation_height(txn.as_mut(), &destination.account());

        let mut receive = destination.receive(txn.txn(), send2.hash()).build();
        ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

        assert_eq!(ctx.ledger.dependents_confirmed(txn.txn(), &receive), true);
    }

    #[test]
    fn dependents_confirmed_pruning() {
        let ctx = LedgerContext::empty();
        let mut txn = ctx.ledger.rw_txn();
        ctx.ledger.enable_pruning();
        let destination = ctx.block_factory();

        let mut send1 = ctx
            .genesis_block_factory()
            .send(txn.txn())
            .amount(Amount::new(1))
            .link(destination.account())
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();
        ctx.inc_confirmation_height(txn.as_mut(), &DEV_GENESIS_ACCOUNT);

        let mut send2 = ctx
            .genesis_block_factory()
            .send(txn.txn())
            .link(destination.account())
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();
        ctx.inc_confirmation_height(txn.as_mut(), &DEV_GENESIS_ACCOUNT);

        assert_eq!(ctx.ledger.pruning_action(txn.as_mut(), &send2.hash(), 1), 2);

        let receive1 = BlockBuilder::state()
            .account(destination.account())
            .previous(0)
            .balance(Amount::new(1))
            .link(send1.hash())
            .sign(&destination.key)
            .build();
        assert_eq!(ctx.ledger.dependents_confirmed(txn.txn(), &receive1), true);
    }
}

mod could_fit {
    use rsnano_core::Epoch;

    use super::*;

    #[test]
    fn legacy_change_block_with_known_previous_block_fits() {
        let ctx = LedgerContext::empty();
        let txn = ctx.ledger.read_txn();
        let change = ctx.genesis_block_factory().legacy_change(txn.txn()).build();
        assert!(ctx.ledger.could_fit(txn.txn(), &change));
    }

    #[test]
    fn change_block_with_known_previous_block_fits() {
        let ctx = LedgerContext::empty();
        let txn = ctx.ledger.read_txn();
        let change2 = ctx.genesis_block_factory().change(txn.txn()).build();
        assert!(ctx.ledger.could_fit(txn.txn(), &change2));
    }

    #[test]
    fn legacy_send_with_unknown_previous_block_does_not_fit() {
        let ctx = LedgerContext::empty();
        let txn = ctx.ledger.read_txn();
        let genesis = ctx.genesis_block_factory();

        let unknown_previous = genesis.legacy_change(txn.txn()).build();

        let send = genesis
            .legacy_send(txn.txn())
            .previous(unknown_previous.hash())
            .build();

        assert_eq!(ctx.ledger.could_fit(txn.txn(), &send), false);
    }

    #[test]
    fn legacy_send_with_known_previous_block_fits() {
        let ctx = LedgerContext::empty();
        let mut txn = ctx.ledger.rw_txn();
        let genesis = ctx.genesis_block_factory();

        let mut known_previous = genesis.legacy_change(txn.txn()).build();
        ctx.ledger
            .process(txn.as_mut(), &mut known_previous)
            .unwrap();

        let send = genesis
            .legacy_send(txn.txn())
            .previous(known_previous.hash())
            .build();

        assert!(ctx.ledger.could_fit(txn.txn(), &known_previous));
        assert!(ctx.ledger.could_fit(txn.txn(), &send));
    }

    #[test]
    fn open_block_for_unknown_send_block_does_not_fit() {
        let ctx = LedgerContext::empty();
        let txn = ctx.ledger.read_txn();
        let genesis = ctx.genesis_block_factory();
        let destination = ctx.block_factory();

        let send = genesis
            .send(txn.txn())
            .amount(Amount::new(1))
            .link(destination.account())
            .build();

        let open = BlockBuilder::state()
            .account(destination.account())
            .previous(0)
            .representative(genesis.account())
            .balance(Amount::new(1))
            .link(send.hash())
            .sign(&destination.key)
            .build();

        assert_eq!(ctx.ledger.could_fit(txn.txn(), &open), false);
    }

    #[test]
    fn legacy_open_block_for_unknown_send_block_does_not_fit() {
        let ctx = LedgerContext::empty();
        let txn = ctx.ledger.read_txn();
        let destination = ctx.block_factory();

        let unknown_send = ctx
            .genesis_block_factory()
            .send(txn.txn())
            .link(destination.account())
            .build();

        let open = destination.legacy_open(unknown_send.hash()).build();

        assert_eq!(ctx.ledger.could_fit(txn.txn(), &open), false);
    }

    #[test]
    fn open_block_for_known_send_block_fits() {
        let ctx = LedgerContext::empty();
        let mut txn = ctx.ledger.rw_txn();
        let destination = ctx.block_factory();

        let mut send = ctx
            .genesis_block_factory()
            .send(txn.txn())
            .link(destination.account())
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

        let open = destination.open(txn.txn(), send.hash()).build();

        assert!(ctx.ledger.could_fit(txn.txn(), &open));
    }

    #[test]
    fn legacy_open_block_for_known_send_block_fits() {
        let ctx = LedgerContext::empty();
        let mut txn = ctx.ledger.rw_txn();
        let destination = ctx.block_factory();

        let mut send = ctx
            .genesis_block_factory()
            .send(txn.txn())
            .link(destination.account())
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

        let open = destination.legacy_open(send.hash()).build();

        assert!(ctx.ledger.could_fit(txn.txn(), &open));
    }

    #[test]
    fn legacy_receive_block_for_unknown_send_block_does_not_fit() {
        let ctx = LedgerContext::empty();
        let mut txn = ctx.ledger.rw_txn();
        let genesis = ctx.genesis_block_factory();
        let destination = ctx.block_factory();

        let mut send1 = genesis.send(txn.txn()).link(destination.account()).build();
        ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

        let mut open = destination.legacy_open(send1.hash()).build();
        ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

        let unknown_send = genesis.send(txn.txn()).link(destination.account()).build();

        let receive = destination
            .legacy_receive(txn.txn(), unknown_send.hash())
            .build();

        assert_eq!(ctx.ledger.could_fit(txn.txn(), &receive), false);
    }

    #[test]
    fn receive_block_for_unknown_send_block_does_not_fit() {
        let ctx = LedgerContext::empty();
        let mut txn = ctx.ledger.rw_txn();
        let genesis = ctx.genesis_block_factory();
        let destination = ctx.block_factory();

        let mut send = genesis
            .send(txn.txn())
            .amount(Amount::new(1))
            .link(destination.account())
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

        let mut open = destination.legacy_open(send.hash()).build();
        ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

        let unknown_send = genesis.send(txn.txn()).link(destination.account()).build();

        let receive = BlockBuilder::state()
            .account(destination.account())
            .previous(open.hash())
            .link(unknown_send.hash())
            .sign(&destination.key)
            .build();

        assert_eq!(ctx.ledger.could_fit(txn.txn(), &receive), false);
    }

    #[test]
    fn legacy_receive_block_for_known_send_block_fits() {
        let ctx = LedgerContext::empty();
        let mut txn = ctx.ledger.rw_txn();
        let genesis = ctx.genesis_block_factory();
        let destination = ctx.block_factory();

        let mut send = genesis
            .send(txn.txn())
            .amount(Amount::new(1))
            .link(destination.account())
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

        let mut open = destination.legacy_open(send.hash()).build();
        ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

        let mut send2 = genesis.send(txn.txn()).link(destination.account()).build();
        ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

        let receive = destination.legacy_receive(txn.txn(), send2.hash()).build();

        assert_eq!(ctx.ledger.could_fit(txn.txn(), &receive), true);
    }

    #[test]
    fn receive_block_for_known_send_block_fits() {
        let ctx = LedgerContext::empty();
        let mut txn = ctx.ledger.rw_txn();
        let genesis = ctx.genesis_block_factory();
        let destination = ctx.block_factory();

        let mut send = genesis.send(txn.txn()).link(destination.account()).build();
        ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

        let mut open = destination.legacy_open(send.hash()).build();
        ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

        let mut send2 = genesis.send(txn.txn()).link(destination.account()).build();
        ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

        let receive = destination.receive(txn.txn(), send2.hash()).build();

        assert_eq!(ctx.ledger.could_fit(txn.txn(), &receive), true);
    }

    #[test]
    fn epoch_v1_block_with_unknown_previous_block_does_not_fit() {
        let ctx = LedgerContext::empty();
        let txn = ctx.ledger.read_txn();
        let genesis = ctx.genesis_block_factory();

        let unknown_send = genesis.send(txn.txn()).build();

        let epoch = BlockBuilder::state()
            .account(genesis.account())
            .previous(unknown_send.hash())
            .representative(unknown_send.representative().unwrap())
            .balance(unknown_send.balance())
            .link(ctx.ledger.epoch_link(Epoch::Epoch1).unwrap())
            .sign(&genesis.key)
            .build();

        assert_eq!(ctx.ledger.could_fit(txn.txn(), &epoch), false);
    }

    #[test]
    fn epoch_v1_block_with_known_previous_block_fits() {
        let ctx = LedgerContext::empty();
        let txn = ctx.ledger.read_txn();

        let epoch = ctx.genesis_block_factory().epoch_v1(txn.txn()).build();

        assert_eq!(ctx.ledger.could_fit(txn.txn(), &epoch), true);
    }
}

#[test]
fn block_confirmed() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    assert_eq!(
        ctx.ledger.block_confirmed(txn.txn(), &DEV_GENESIS_HASH),
        true
    );

    let destination = ctx.block_factory();
    let mut send = ctx
        .genesis_block_factory()
        .send(txn.txn())
        .link(destination.account())
        .build();

    // Must be safe against non-existing blocks
    assert_eq!(ctx.ledger.block_confirmed(txn.txn(), &send.hash()), false);

    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();
    assert_eq!(ctx.ledger.block_confirmed(txn.txn(), &send.hash()), false);

    ctx.inc_confirmation_height(txn.as_mut(), &DEV_GENESIS_ACCOUNT);
    assert_eq!(ctx.ledger.block_confirmed(txn.txn(), &send.hash()), true);
}

#[test]
fn ledger_cache() {
    let ctx = LedgerContext::empty();
    let genesis = ctx.genesis_block_factory();
    let total = 10u64;

    struct ExpectedCache {
        account_count: u64,
        block_count: u64,
        cemented_count: u64,
        genesis_weight: Amount,
        pruned_count: u64,
    }

    // Check existing ledger (incremental cache update) and reload on a new ledger
    for i in 0..total {
        let mut expected = ExpectedCache {
            account_count: 1 + i,
            block_count: 1 + 2 * (i + 1) - 2,
            cemented_count: 1 + 2 * (i + 1) - 2,
            genesis_weight: LEDGER_CONSTANTS_STUB.genesis_amount - Amount::new(i as u128),
            pruned_count: i,
        };

        let check_impl = |cache: &LedgerCache, expected: &ExpectedCache| {
            assert_eq!(
                cache.account_count.load(Ordering::Relaxed),
                expected.account_count
            );
            assert_eq!(
                cache.block_count.load(Ordering::Relaxed),
                expected.block_count
            );
            assert_eq!(
                cache.cemented_count.load(Ordering::Relaxed),
                expected.cemented_count
            );
            assert_eq!(
                cache.rep_weights.representation_get(&DEV_GENESIS_ACCOUNT),
                expected.genesis_weight
            );
            assert_eq!(
                cache.pruned_count.load(Ordering::Relaxed),
                expected.pruned_count
            );
        };

        let cache_check = |cache: &LedgerCache, expected: &ExpectedCache| {
            check_impl(cache, expected);

            let new_ledger =
                Ledger::new(ctx.ledger.store.clone(), LEDGER_CONSTANTS_STUB.clone()).unwrap();
            check_impl(&new_ledger.cache, expected);
        };

        let destination = ctx.block_factory();
        let send = {
            let mut txn = ctx.ledger.rw_txn();
            let mut send = genesis.send(txn.txn()).link(destination.account()).build();
            ctx.ledger.process(txn.as_mut(), &mut send).unwrap();
            expected.block_count += 1;
            expected.genesis_weight = send.balance();
            send
        };
        cache_check(&ctx.ledger.cache, &expected);

        let open = {
            let mut txn = ctx.ledger.rw_txn();
            let mut open = destination.open(txn.txn(), send.hash()).build();
            ctx.ledger.process(txn.as_mut(), &mut open).unwrap();
            expected.block_count += 1;
            expected.account_count += 1;
            open
        };
        cache_check(&ctx.ledger.cache, &expected);

        {
            let mut txn = ctx.ledger.rw_txn();
            ctx.inc_confirmation_height(txn.as_mut(), &DEV_GENESIS_ACCOUNT);
            ctx.ledger
                .cache
                .cemented_count
                .fetch_add(1, Ordering::Relaxed);
            expected.cemented_count += 1;
        }
        cache_check(&ctx.ledger.cache, &expected);

        {
            let mut txn = ctx.ledger.rw_txn();
            ctx.inc_confirmation_height(txn.as_mut(), &destination.account());
            ctx.ledger
                .cache
                .cemented_count
                .fetch_add(1, Ordering::Relaxed);
            expected.cemented_count += 1;
        }
        cache_check(&ctx.ledger.cache, &expected);

        {
            let mut txn = ctx.ledger.rw_txn();
            ctx.ledger.store.pruned().put(txn.as_mut(), &open.hash());
            ctx.ledger
                .cache
                .pruned_count
                .fetch_add(1, Ordering::Relaxed);
            expected.pruned_count += 1;
        }
        cache_check(&ctx.ledger.cache, &expected);
    }
}

#[test]
fn unconfirmed_frontiers() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    assert!(ctx.ledger.unconfirmed_frontiers().is_empty());

    let genesis = ctx.genesis_block_factory();
    let destination = ctx.block_factory();
    let latest = ctx.ledger.latest(txn.txn(), &genesis.account()).unwrap();

    let mut send = genesis.send(txn.txn()).link(destination.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();
    txn.commit();

    let unconfirmed_frontiers = ctx.ledger.unconfirmed_frontiers();
    assert_eq!(unconfirmed_frontiers.len(), 1);
    let (key, value) = unconfirmed_frontiers.iter().next().unwrap();
    assert_eq!(*key, 1);
    assert_eq!(
        value.first().unwrap(),
        &UncementedInfo {
            cemented_frontier: latest,
            frontier: send.hash(),
            account: genesis.account()
        }
    )
}

#[test]
fn is_send_genesis() {
    let ctx = LedgerContext::empty();
    let txn = ctx.ledger.read_txn();
    assert_eq!(
        ctx.ledger
            .is_send(txn.txn(), DEV_GENESIS.read().unwrap().deref().deref()),
        false
    );
}

#[test]
fn is_send_state() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let open = setup_open_block(&ctx, txn.as_mut());
    assert_eq!(
        ctx.ledger
            .is_send(txn.txn(), open.send_block.deref().deref()),
        true
    );
    assert_eq!(
        ctx.ledger
            .is_send(txn.txn(), open.open_block.deref().deref()),
        false
    );
}

#[test]
fn is_send_legacy() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let open = setup_legacy_open_block(&ctx, txn.as_mut());
    assert_eq!(
        ctx.ledger
            .is_send(txn.txn(), open.send_block.deref().deref()),
        true
    );
    assert_eq!(
        ctx.ledger
            .is_send(txn.txn(), open.open_block.deref().deref()),
        false
    );
}

#[test]
fn sideband_height() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let dest1 = ctx.block_factory();
    let dest2 = ctx.block_factory();
    let dest3 = ctx.block_factory();

    let mut send = genesis
        .legacy_send(txn.txn())
        .destination(genesis.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut receive = genesis.legacy_receive(txn.txn(), send.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

    let mut change = genesis.legacy_change(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut change).unwrap();

    let mut state_send1 = genesis.send(txn.txn()).link(dest1.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut state_send1).unwrap();

    let mut state_send2 = genesis.send(txn.txn()).link(dest2.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut state_send2).unwrap();

    let mut state_send3 = genesis.send(txn.txn()).link(dest3.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut state_send3).unwrap();

    let mut state_open = dest1.open(txn.txn(), state_send1.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut state_open).unwrap();

    let mut epoch = dest1.epoch_v1(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut epoch).unwrap();

    let mut epoch_open = dest2.epoch_v1_open().build();
    ctx.ledger.process(txn.as_mut(), &mut epoch_open).unwrap();

    let mut state_receive = dest2.receive(txn.txn(), state_send2.hash()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut state_receive)
        .unwrap();

    let mut open = dest3.legacy_open(state_send3.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

    let assert_sideband_height = |hash: &BlockHash, expected_height: u64| {
        let block = ctx.ledger.get_block(txn.txn(), hash).unwrap();
        assert_eq!(block.sideband().unwrap().height, expected_height);
    };

    assert_sideband_height(&DEV_GENESIS_HASH, 1);
    assert_sideband_height(&send.hash(), 2);
    assert_sideband_height(&receive.hash(), 3);
    assert_sideband_height(&change.hash(), 4);
    assert_sideband_height(&state_send1.hash(), 5);
    assert_sideband_height(&state_send2.hash(), 6);
    assert_sideband_height(&state_send3.hash(), 7);

    assert_sideband_height(&state_open.hash(), 1);
    assert_sideband_height(&epoch.hash(), 2);

    assert_sideband_height(&epoch_open.hash(), 1);
    assert_sideband_height(&state_receive.hash(), 2);

    assert_sideband_height(&open.hash(), 1);
}
