use crate::{
    block_insertion::{BlockInsertInstructions, BlockValidatorFactory},
    Ledger, ProcessResult,
};
use rsnano_core::{
    Account, AccountInfo, Amount, BlockBuilder, BlockDetails, BlockEnum, BlockHash, BlockSideband,
    Epoch, KeyPair, PendingInfo, PendingKey, StateBlockBuilder,
};
use rsnano_store_lmdb::EnvironmentStub;

#[test]
fn valid_send_block() {
    let (instructions, keypair, _) = insert_send_block();
    assert_eq!(instructions.unwrap().account, keypair.public_key());
}

#[test]
fn insert_pending_info() {
    let (instructions, _, send) = insert_send_block();
    let instructions = instructions.unwrap();

    assert_eq!(instructions.delete_pending, None);
    assert_eq!(
        instructions.insert_pending,
        Some((
            PendingKey::new(send.destination_or_link(), send.hash()),
            PendingInfo {
                amount: instructions.old_account_info.balance - send.balance(),
                epoch: Epoch::Epoch0,
                source: instructions.account
            }
        ))
    );
}

#[test]
fn create_sideband() {
    let (instructions, keypair, _) = insert_send_block();
    let sideband = instructions.unwrap().set_sideband;

    assert_eq!(sideband.height, 2);
    assert_eq!(sideband.account, keypair.public_key());
    assert_eq!(
        sideband.details,
        BlockDetails::new(Epoch::Epoch0, true, false, false)
    );
}

#[test]
fn send_and_change_representative() {
    let (ledger, open_block, keypair, account_info) = create_ledger_with_open_block();
    let new_representative = Account::from(555555);
    let send = create_send_block(open_block, account_info, &keypair)
        .representative(new_representative)
        .build();
    let (instructions, _) = process_block(ledger, send);

    assert_eq!(
        instructions.unwrap().set_account_info.representative,
        new_representative
    );
}

#[test]
fn send_to_burn_account() {
    let (ledger, open_block, keypair, account_info) = create_ledger_with_open_block();
    let send = create_send_block(open_block, account_info, &keypair)
        .link(0)
        .build();
    let (instructions, _) = process_block(ledger, send);

    assert!(instructions.is_ok())
}

fn insert_send_block() -> (
    Result<BlockInsertInstructions, ProcessResult>,
    KeyPair,
    BlockEnum,
) {
    let (ledger, open_block, keypair, account_info) = create_ledger_with_open_block();
    let send = create_send_block(open_block, account_info, &keypair).build();
    let result = process_block(ledger, send);
    (result.0, keypair, result.1)
}

fn process_block(
    ledger: Ledger<EnvironmentStub>,
    send: BlockEnum,
) -> (Result<BlockInsertInstructions, ProcessResult>, BlockEnum) {
    let txn = ledger.store.tx_begin_read();
    let validator = BlockValidatorFactory::new(&ledger, &txn, &send).create_validator();
    let instructions = validator.validate();
    (instructions, send)
}

fn create_ledger_with_open_block() -> (Ledger<EnvironmentStub>, BlockEnum, KeyPair, AccountInfo) {
    let (open_block, keypair, account_info) = create_legacy_open_block();

    let ledger = Ledger::create_null_with()
        .block(&open_block)
        .frontier(&open_block.hash(), &open_block.account())
        .account_info(&keypair.public_key(), &account_info)
        .build();
    (ledger, open_block, keypair, account_info)
}

fn create_send_block(
    open_block: BlockEnum,
    account_info: AccountInfo,
    keypair: &KeyPair,
) -> StateBlockBuilder {
    BlockBuilder::state()
        .account(open_block.account())
        .representative(open_block.representative().unwrap())
        .previous(open_block.hash())
        .balance(account_info.balance - Amount::raw(1))
        .sign(keypair)
}

fn create_legacy_open_block() -> (BlockEnum, KeyPair, AccountInfo) {
    let keypair = KeyPair::new();

    let mut open_block = BlockBuilder::legacy_open()
        .account(keypair.public_key())
        .sign(&keypair)
        .build();

    open_block.set_sideband(BlockSideband {
        height: 1,
        successor: BlockHash::zero(),
        account: keypair.public_key(),
        ..BlockSideband::create_test_instance()
    });

    let account_info = AccountInfo {
        head: open_block.hash(),
        open_block: open_block.hash(),
        block_count: 1,
        epoch: Epoch::Epoch0,
        ..AccountInfo::create_test_instance()
    };

    (open_block, keypair, account_info)
}
