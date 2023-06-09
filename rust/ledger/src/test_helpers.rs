use rsnano_core::{
    Account, AccountInfo, BlockBuilder, BlockDetails, BlockEnum, BlockSideband, Epoch, KeyPair,
    LegacyReceiveBlockBuilder, StateBlockBuilder, DEV_GENESIS_KEY, };

use crate::ledger_constants::LEDGER_CONSTANTS_STUB;

pub(crate) fn create_test_account_info(block: &BlockEnum) -> AccountInfo {
    AccountInfo {
        balance: block.balance_calculated(),
        head: block.hash(),
        epoch: block
            .sideband()
            .map(|sb| sb.details.epoch)
            .unwrap_or(Epoch::Epoch0),
        representative: block.representative().unwrap_or(Account::from(2)),
        ..AccountInfo::create_test_instance()
    }
}

pub(crate) fn create_state_block(epoch: Epoch) -> (KeyPair, BlockEnum) {
    let keypair = KeyPair::new();
    let mut state = BlockBuilder::state()
        .account(keypair.public_key())
        .link(0)
        .balance(1000)
        .sign(&keypair)
        .build();
    state.set_sideband(BlockSideband {
        account: keypair.public_key(),
        details: BlockDetails::new(epoch, false, true, false),
        ..BlockSideband::create_test_instance()
    });

    (keypair, state)
}

pub(crate) fn create_legacy_open_block() -> (KeyPair, BlockEnum) {
    let keypair = KeyPair::new();
    let mut open = BlockBuilder::legacy_open()
        .account(keypair.public_key())
        .sign(&keypair)
        .build();
    open.set_sideband(BlockSideband {
        details: BlockDetails::new(Epoch::Epoch0, false, true, false),
        ..BlockSideband::create_test_instance()
    });
    (keypair, open)
}

pub(crate) fn epoch_successor(previous: &BlockEnum, epoch: Epoch) -> StateBlockBuilder {
    BlockBuilder::state()
        .account(previous.account_calculated())
        .balance(previous.balance_calculated())
        .representative(previous.representative().unwrap())
        .link(*LEDGER_CONSTANTS_STUB.epochs.link(epoch).unwrap())
        .previous(previous.hash())
        .sign(&DEV_GENESIS_KEY)
}

pub(crate) fn state_successor(keypair: KeyPair, previous: &BlockEnum) -> StateBlockBuilder {
    BlockBuilder::state()
        .account(keypair.public_key())
        .previous(previous.hash())
        .link(0)
        .sign(&keypair)
}

pub(crate) fn legacy_receive_successor(
    keypair: KeyPair,
    previous: &BlockEnum,
) -> LegacyReceiveBlockBuilder {
    BlockBuilder::legacy_receive()
        .previous(previous.hash())
        .sign(&keypair)
}

pub(crate) fn epoch_block_sideband(epoch: Epoch) -> BlockSideband {
    BlockSideband {
        details: BlockDetails::new(epoch, false, false, true),
        source_epoch: epoch,
        ..BlockSideband::create_test_instance()
    }
}
