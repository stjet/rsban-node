#[cfg(test)]
use rsnano_core::BlockChainBuilder;
#[cfg(test)]
use rsnano_core::Epoch;
use rsnano_store_lmdb::Transaction;
use std::cell::Cell;
#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::collections::HashSet;

use rsnano_core::{Account, AccountInfo, BlockEnum, BlockHash, ConfirmationHeightInfo};
use rsnano_ledger::Ledger;

pub trait LedgerDataRequester {
    fn get_block(&mut self, block_hash: &BlockHash) -> Option<BlockEnum>;
    fn was_block_pruned(&self, block_hash: &BlockHash) -> bool;
    fn get_confirmation_height(&self, account: &Account) -> Option<ConfirmationHeightInfo>;
    fn get_account_info(&self, account: &Account) -> Option<AccountInfo>;
}

/** The maximum number of blocks to be read in while iterating over a long account chain */
const BATCH_READ_SIZE: usize = 65536;

pub(crate) struct LedgerAdapter<'a> {
    txn: &'a mut dyn Transaction,
    ledger: &'a Ledger,
    block_read_count: Cell<usize>,
    max_block_read_count: usize,
}

impl<'a> LedgerAdapter<'a> {
    pub(crate) fn new(txn: &'a mut dyn Transaction, ledger: &'a Ledger) -> Self {
        Self {
            txn,
            ledger,
            block_read_count: Cell::new(0),
            max_block_read_count: BATCH_READ_SIZE,
        }
    }

    pub(crate) fn new_unlimited(txn: &'a mut dyn Transaction, ledger: &'a Ledger) -> Self {
        Self {
            txn,
            ledger,
            block_read_count: Cell::new(0),
            max_block_read_count: usize::MAX,
        }
    }

    pub fn refresh_transaction(&mut self) {
        self.txn.refresh();
    }
}

impl<'a> LedgerDataRequester for LedgerAdapter<'a> {
    fn get_block(&mut self, block_hash: &BlockHash) -> Option<BlockEnum> {
        let read_count = self.block_read_count.get();
        if read_count > 0 && read_count % self.max_block_read_count == 0 {
            // We could be traversing a very large account so we don't want to open read transactions for too long.
            self.txn.refresh();
        }

        self.block_read_count.set(read_count + 1);
        self.ledger.store.block.get(self.txn, block_hash)
    }

    fn get_confirmation_height(&self, account: &Account) -> Option<ConfirmationHeightInfo> {
        self.ledger.store.confirmation_height.get(self.txn, account)
    }

    fn was_block_pruned(&self, block_hash: &BlockHash) -> bool {
        self.ledger.pruning_enabled() && self.ledger.store.pruned.exists(self.txn, block_hash)
    }

    fn get_account_info(&self, account: &Account) -> Option<AccountInfo> {
        self.ledger.account_info(self.txn, account)
    }
}

#[cfg(test)]
pub(crate) struct LedgerDataRequesterStub {
    blocks: HashMap<BlockHash, BlockEnum>,
    confirmation_heights: HashMap<Account, ConfirmationHeightInfo>,
    account_infos: HashMap<Account, AccountInfo>,
    pruned: HashSet<BlockHash>,
    blocks_loaded: Cell<usize>,
    confirmation_heights_loaded: Cell<usize>,
}

#[cfg(test)]
impl LedgerDataRequesterStub {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            confirmation_heights: HashMap::new(),
            account_infos: HashMap::new(),
            pruned: HashSet::new(),
            blocks_loaded: Cell::new(0),
            confirmation_heights_loaded: Cell::new(0),
        }
    }

    fn add_block(&mut self, block: BlockEnum) {
        let info = self
            .account_infos
            .entry(block.account_calculated())
            .or_insert_with(|| AccountInfo {
                head: block.hash(),
                representative: block.representative().unwrap_or_default(),
                open_block: block.hash(),
                balance: block.balance_calculated(),
                modified: 0,
                block_count: 1,
                epoch: Epoch::Unspecified,
            });
        info.head = block.hash();
        info.block_count = block.sideband().unwrap().height;
        self.blocks.insert(block.hash(), block);
    }

    pub fn set_confirmation_height(&mut self, account: Account, info: ConfirmationHeightInfo) {
        self.confirmation_heights.insert(account, info);
    }

    pub fn set_account_info(&mut self, account: Account, info: AccountInfo) {
        self.account_infos.insert(account, info);
    }

    pub fn add_genesis_block(&mut self) -> BlockChainBuilder {
        let mut genesis_chain = BlockChainBuilder::for_account(123456).legacy_open();
        self.add_cemented(&mut genesis_chain);
        genesis_chain
    }

    pub fn add_cemented(&mut self, chain: &BlockChainBuilder) {
        self.set_confirmation_height(
            chain.account(),
            ConfirmationHeightInfo {
                height: chain.height(),
                frontier: chain.frontier(),
            },
        );
        self.add_uncemented(chain);
    }

    pub fn add_uncemented(&mut self, chain: &BlockChainBuilder) {
        for block in chain.blocks() {
            self.add_block(block.clone());
        }
    }

    pub fn cement(&mut self, block: &BlockEnum) {
        let account = block.account_calculated();
        let current_info = self.get_confirmation_height(&account).unwrap_or_default();
        assert!(current_info.height < block.height());
        self.set_confirmation_height(
            account,
            ConfirmationHeightInfo {
                height: block.height(),
                frontier: block.hash(),
            },
        );
    }

    pub fn prune(&mut self, hash: BlockHash) {
        self.pruned.insert(hash);
        self.blocks.remove(&hash);
    }

    pub fn blocks_loaded(&self) -> usize {
        self.blocks_loaded.get()
    }

    pub fn confirmation_heights_loaded(&self) -> usize {
        self.confirmation_heights_loaded.get()
    }
}

#[cfg(test)]
impl LedgerDataRequester for LedgerDataRequesterStub {
    fn get_block(&mut self, block_hash: &BlockHash) -> Option<BlockEnum> {
        self.blocks_loaded.set(self.blocks_loaded.get() + 1);
        self.blocks.get(block_hash).cloned()
    }

    fn was_block_pruned(&self, block_hash: &BlockHash) -> bool {
        self.pruned.contains(block_hash)
    }

    fn get_confirmation_height(&self, account: &Account) -> Option<ConfirmationHeightInfo> {
        self.confirmation_heights_loaded
            .set(self.confirmation_heights_loaded.get() + 1);
        self.confirmation_heights.get(account).cloned()
    }

    fn get_account_info(&self, account: &Account) -> Option<AccountInfo> {
        self.account_infos.get(account).cloned()
    }
}

#[cfg(test)]
mod tests {
    use rsnano_core::BlockBuilder;

    use super::*;

    #[test]
    fn empty_cementation_data_requester_stub() {
        let mut stub = LedgerDataRequesterStub::new();
        assert_eq!(stub.get_block(&BlockHash::from(1)), None);
        assert_eq!(stub.was_block_pruned(&BlockHash::from(1)), false);
        assert_eq!(
            stub.get_confirmation_height(&Account::from(1)),
            Default::default()
        );
        assert_eq!(stub.get_account_info(&Account::from(1)), None);
    }

    #[test]
    fn add_block_to_cementation_data_requester_stub() {
        let mut stub = LedgerDataRequesterStub::new();
        let block = BlockBuilder::state().with_sideband().build();
        stub.add_block(block.clone());
        assert_eq!(stub.get_block(&block.hash()), Some(block));
    }

    #[test]
    fn set_confirmation_height() {
        let mut stub = LedgerDataRequesterStub::new();
        let account = Account::from(1);
        let confirmation_height = ConfirmationHeightInfo::test_instance();

        stub.set_confirmation_height(account, confirmation_height.clone());

        assert_eq!(
            stub.get_confirmation_height(&account),
            Some(confirmation_height)
        );
    }

    #[test]
    fn set_account_info() {
        let mut stub = LedgerDataRequesterStub::new();
        let account = Account::from(1);
        let account_info = AccountInfo::test_instance();

        stub.set_account_info(account, account_info.clone());

        assert_eq!(stub.get_account_info(&account), Some(account_info));
    }

    #[test]
    fn prune() {
        let mut stub = LedgerDataRequesterStub::new();
        let block = BlockBuilder::legacy_open().with_sideband().build();
        let hash = block.hash();
        stub.add_block(block);
        stub.prune(hash);
        assert_eq!(stub.was_block_pruned(&hash), true);
        assert_eq!(stub.was_block_pruned(&BlockHash::from(42)), false);
    }
}
