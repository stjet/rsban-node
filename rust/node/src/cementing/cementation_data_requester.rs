#[cfg(test)]
use std::collections::HashMap;

#[cfg(test)]
use rsnano_core::BlockBuilder;
use rsnano_core::{Account, AccountInfo, BlockEnum, BlockHash, ConfirmationHeightInfo};
#[cfg(test)]
use rsnano_core::{Amount, BlockDetails, BlockSideband, Epoch, LegacySendBlockBuilder};
use rsnano_ledger::Ledger;
use rsnano_store_traits::Transaction;

pub(crate) trait CementationDataRequester {
    fn get_block(&self, block_hash: &BlockHash) -> Option<BlockEnum>;
    fn was_block_pruned(&self, block_hash: &BlockHash) -> bool;
    fn get_current_confirmation_height(&self, account: &Account) -> ConfirmationHeightInfo;
    fn get_account_info(&self, account: &Account) -> Option<AccountInfo>;
    fn refresh_transaction(&mut self);
}

pub(crate) struct CementationLedgerAdapter<'a> {
    txn: &'a mut dyn Transaction,
    ledger: &'a Ledger,
}

impl<'a> CementationLedgerAdapter<'a> {
    pub(crate) fn new(txn: &'a mut dyn Transaction, ledger: &'a Ledger) -> Self {
        Self { txn, ledger }
    }
}

impl<'a> CementationDataRequester for CementationLedgerAdapter<'a> {
    fn get_block(&self, block_hash: &BlockHash) -> Option<BlockEnum> {
        self.ledger.store.block().get(self.txn, block_hash)
    }

    fn get_current_confirmation_height(&self, account: &Account) -> ConfirmationHeightInfo {
        self.ledger
            .store
            .confirmation_height()
            .get(self.txn, account)
            .unwrap_or_default()
    }

    fn was_block_pruned(&self, block_hash: &BlockHash) -> bool {
        self.ledger.pruning_enabled() && self.ledger.store.pruned().exists(self.txn, block_hash)
    }

    fn get_account_info(&self, account: &Account) -> Option<AccountInfo> {
        self.ledger.account_info(self.txn, account)
    }

    fn refresh_transaction(&mut self) {
        self.txn.refresh();
    }
}

#[cfg(test)]
pub(crate) struct CementationDataRequesterStub {
    blocks: HashMap<BlockHash, BlockEnum>,
    confirmation_heights: HashMap<Account, ConfirmationHeightInfo>,
    account_infos: HashMap<Account, AccountInfo>,
}

#[cfg(test)]
impl CementationDataRequesterStub {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            confirmation_heights: HashMap::new(),
            account_infos: HashMap::new(),
        }
    }
    pub fn add_block(&mut self, block: BlockEnum) {
        self.blocks.insert(block.hash(), block);
    }

    pub fn set_confirmation_height(&mut self, account: Account, info: ConfirmationHeightInfo) {
        self.confirmation_heights.insert(account, info);
    }

    pub fn set_account_info(&mut self, account: Account, info: AccountInfo) {
        self.account_infos.insert(account, info);
    }

    pub fn add_cemented(&mut self, chain: &mut BlockChainBuilder) {
        self.set_confirmation_height(
            chain.account,
            ConfirmationHeightInfo {
                height: chain.height(),
                frontier: chain.frontier(),
            },
        );
        self.add_uncemented(chain);
    }

    pub fn add_uncemented(&mut self, chain: &mut BlockChainBuilder) {
        for block in chain.take_blocks() {
            self.blocks.insert(block.hash(), block);
        }
    }

    pub(crate) fn cement(&mut self, hash: &BlockHash) {
        let block = self.blocks.get(hash).unwrap();
        let sideband = block.sideband().unwrap();
        self.set_confirmation_height(
            block.account_calculated(),
            ConfirmationHeightInfo {
                height: sideband.height,
                frontier: block.hash(),
            },
        )
    }
}

#[cfg(test)]
pub(crate) struct BlockChainBuilder {
    pub account: Account,
    blocks: Vec<BlockEnum>,
    height: u64,
    frontier: BlockHash,
}

#[cfg(test)]
impl BlockChainBuilder {
    pub(crate) fn new(account: Account) -> Self {
        Self {
            account,
            blocks: Vec::new(),
            height: 0,
            frontier: BlockHash::zero(),
        }
    }

    fn add_block(&mut self, mut block: BlockEnum) -> &BlockEnum {
        block.set_sideband(BlockSideband {
            height: self.height + 1,
            timestamp: 1,
            successor: BlockHash::zero(),
            account: self.account,
            balance: Amount::zero(),
            details: BlockDetails::new(Epoch::Unspecified, false, false, false),
            source_epoch: rsnano_core::Epoch::Unspecified,
        });

        if self.blocks.len() > 0 {
            let previous = self.blocks.last_mut().unwrap();
            let mut sideband = previous.sideband().unwrap().clone();
            sideband.successor = block.hash();
            previous.set_sideband(sideband);
        }

        self.height += 1;
        self.frontier = block.hash();
        self.blocks.push(block);
        self.blocks.last().unwrap()
    }

    pub fn legacy_open(&mut self) -> &BlockEnum {
        let block_builder = BlockBuilder::legacy_open().account(self.account);
        self.add_block(block_builder.build())
    }

    pub fn legacy_open_from(&mut self, send: &BlockEnum) -> &BlockEnum {
        assert_eq!(send.destination_or_link(), self.account);
        let block_builder = BlockBuilder::legacy_open()
            .account(self.account)
            .source(send.hash());
        self.add_block(block_builder.build())
    }

    pub fn legacy_send(&mut self) -> &BlockEnum {
        self.legacy_send_with(|b| b)
    }

    pub fn legacy_send_with<F: FnMut(LegacySendBlockBuilder) -> LegacySendBlockBuilder>(
        &mut self,
        mut f: F,
    ) -> &BlockEnum {
        let block_builder = BlockBuilder::legacy_send()
            .account(self.account)
            .previous(self.frontier);
        self.add_block(f(block_builder).build())
    }

    pub fn take_blocks(&mut self) -> Vec<BlockEnum> {
        let mut blocks = Vec::new();
        std::mem::swap(&mut blocks, &mut self.blocks);
        blocks
    }

    fn height(&self) -> u64 {
        self.height
    }

    fn frontier(&self) -> BlockHash {
        self.frontier
    }
}

#[cfg(test)]
impl CementationDataRequester for CementationDataRequesterStub {
    fn get_block(&self, block_hash: &BlockHash) -> Option<BlockEnum> {
        self.blocks.get(block_hash).cloned()
    }

    fn was_block_pruned(&self, _block_hash: &BlockHash) -> bool {
        false
    }

    fn get_current_confirmation_height(&self, account: &Account) -> ConfirmationHeightInfo {
        self.confirmation_heights
            .get(account)
            .cloned()
            .unwrap_or_default()
    }

    fn get_account_info(&self, account: &Account) -> Option<AccountInfo> {
        self.account_infos.get(account).cloned()
    }

    fn refresh_transaction(&mut self) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use rsnano_core::BlockBuilder;

    use super::*;

    #[test]
    fn empty_cementation_data_requester_stub() {
        let stub = CementationDataRequesterStub::new();
        assert_eq!(stub.get_block(&BlockHash::from(1)), None);
        assert_eq!(stub.was_block_pruned(&BlockHash::from(1)), false);
        assert_eq!(
            stub.get_current_confirmation_height(&Account::from(1)),
            Default::default()
        );
        assert_eq!(stub.get_account_info(&Account::from(1)), None);
    }

    #[test]
    fn add_block_to_cementation_data_requester_stub() {
        let mut stub = CementationDataRequesterStub::new();
        let block = BlockBuilder::state().build();
        stub.add_block(block.clone());
        assert_eq!(stub.get_block(&block.hash()), Some(block));
    }

    #[test]
    fn set_confirmation_height() {
        let mut stub = CementationDataRequesterStub::new();
        let account = Account::from(1);
        let confirmation_height = ConfirmationHeightInfo::test_instance();

        stub.set_confirmation_height(account, confirmation_height.clone());

        assert_eq!(
            stub.get_current_confirmation_height(&account),
            confirmation_height
        );
    }

    #[test]
    fn set_account_info() {
        let mut stub = CementationDataRequesterStub::new();
        let account = Account::from(1);
        let account_info = AccountInfo::test_instance();

        stub.set_account_info(account, account_info.clone());

        assert_eq!(stub.get_account_info(&account), Some(account_info));
    }
}
