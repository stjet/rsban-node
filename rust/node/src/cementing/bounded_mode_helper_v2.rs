use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::{Account, BlockEnum, BlockHash, ConfirmationHeightInfo, Epochs};
use rsnano_ledger::DEV_GENESIS;

use super::{
    accounts_confirmed_map::ConfirmedInfo, ledger_data_requester::LedgerDataRequester,
    AccountsConfirmedMap, AccountsConfirmedMapContainerInfo, WriteDetails,
};

/** The maximum number of blocks to be read in while iterating over a long account chain */
const BATCH_READ_SIZE: u64 = 65536;

/** The maximum number of various containers to keep the memory bounded */
const MAX_ITEMS: usize = 131072;

#[derive(PartialEq, Eq, Debug)]
pub(crate) enum BoundedCementationStep {
    Write(WriteDetails),
    AlreadyCemented(BlockHash),
    Done,
}
struct BlockRange {
    // inclusive lowest block
    bottom: BlockHash,
    // inclusive highest block
    top: BlockHash,
}

impl BlockRange {
    fn new(bottom: BlockHash, top: BlockHash) -> Self {
        Self { bottom, top }
    }
}

// Data for iterating a single block chain
#[derive(Debug)]
struct ChainIteration {
    account: Account,
    bottom_hash: BlockHash,
    bottom_height: u64,
    top_hash: BlockHash,
    top_height: u64,
    current_hash: BlockHash,
    current_height: u64,
    /// The block after the highest block which we cement. This will become the new lowest uncemented block.
    top_successor: Option<BlockHash>,
}

impl ChainIteration {
    fn new(lowest_block: &BlockEnum, highest_block: &BlockEnum) -> Self {
        debug_assert!(lowest_block.height() <= highest_block.height());
        Self {
            account: highest_block.account_calculated(),
            bottom_hash: lowest_block.hash(),
            bottom_height: lowest_block.height(),
            top_hash: highest_block.hash(),
            top_height: highest_block.height(),
            current_hash: lowest_block.hash(),
            current_height: lowest_block.height(),
            top_successor: highest_block.successor(),
        }
    }

    fn set_done(&mut self) {
        self.current_hash = BlockHash::zero();
        self.current_height = self.top_height + 1;
    }

    fn is_done(&self) -> bool {
        self.current_height > self.top_height
    }

    /// Search range for receive blocks
    fn search_range(&self) -> BlockRange {
        BlockRange {
            bottom: self.current_hash,
            top: self.top_hash,
        }
    }

    fn go_to_successor_of(&mut self, block: &BlockEnum) {
        self.current_hash = block.successor().unwrap_or_default();
        self.current_height = block.height() + 1;
    }

    fn into_write_details(&self) -> WriteDetails {
        WriteDetails {
            account: self.account,
            bottom_height: self.bottom_height,
            bottom_hash: self.bottom_hash,
            top_height: self.top_height,
            top_hash: self.top_hash,
        }
    }
}

#[derive(Default)]
pub(crate) struct BoundedModeHelperV2Builder {
    epochs: Option<Epochs>,
    stopped: Option<Arc<AtomicBool>>,
    max_items: Option<usize>,
}

impl BoundedModeHelperV2Builder {
    pub fn epochs(mut self, epochs: Epochs) -> Self {
        self.epochs = Some(epochs);
        self
    }

    pub fn stopped(mut self, stopped: Arc<AtomicBool>) -> Self {
        self.stopped = Some(stopped);
        self
    }

    pub fn max_items(mut self, max: usize) -> Self {
        self.max_items = Some(max);
        self
    }

    pub fn build(self) -> BoundedModeHelperV2 {
        let epochs = self.epochs.unwrap_or_default();
        let stopped = self
            .stopped
            .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));

        BoundedModeHelperV2::new(epochs, stopped, self.max_items.unwrap_or(MAX_ITEMS))
    }
}

pub(crate) struct BoundedModeHelperV2 {
    stopped: Arc<AtomicBool>,
    epochs: Epochs,
    chain_stack: BoundedVecDeque<ChainIteration>,
    chains_encountered: usize,
    confirmation_heights: AccountsConfirmedMap,
    original_block: BlockEnum,
    checkpoints: BoundedVecDeque<BlockHash>,
    latest_cementation: BlockHash,
    block_read_count: u64,
}

impl BoundedModeHelperV2 {
    pub fn new(epochs: Epochs, stopped: Arc<AtomicBool>, max_items: usize) -> Self {
        Self {
            epochs,
            stopped,
            chain_stack: BoundedVecDeque::new(max_items),
            confirmation_heights: AccountsConfirmedMap::new(),
            chains_encountered: 0,
            original_block: DEV_GENESIS.read().unwrap().clone(), //todo
            checkpoints: BoundedVecDeque::new(max_items),
            latest_cementation: BlockHash::zero(),
            block_read_count: 0,
        }
    }

    pub fn builder() -> BoundedModeHelperV2Builder {
        Default::default()
    }

    pub fn initialize(&mut self, original_block: BlockEnum) {
        self.latest_cementation = BlockHash::zero();
        self.chain_stack.clear();
        self.chains_encountered = 0;
        self.checkpoints.clear();
        self.original_block = original_block;
        self.block_read_count = 0;
    }

    pub fn get_next_step<T: LedgerDataRequester>(
        &mut self,
        data_requester: &mut T,
    ) -> anyhow::Result<BoundedCementationStep> {
        //todo don't load original block from db
        //todo add optimizations: cement 2 blocks without db lookup
        loop {
            if self.stopped.load(Ordering::Relaxed) {
                return Ok(BoundedCementationStep::Done);
            }
            self.restore_checkpoint_if_required(data_requester)?;
            let Some(chain) = self.chain_stack.back() else { break; };

            if chain.is_done() {
                // There is nothing left to do for this chain. We can write the confirmation height now.
                let chain = self.chain_stack.pop_back().unwrap();
                if self.checkpoints.back() == Some(&chain.top_hash) {
                    self.checkpoints.pop_back();
                }
                let new_first_unconfirmed = chain.top_successor;
                if let Some(write_details) = self.get_write_details(&chain) {
                    self.cache_confirmation_height(&write_details, new_first_unconfirmed);
                    self.latest_cementation = write_details.top_hash;
                    return Ok(BoundedCementationStep::Write(write_details));
                }
            } else {
                self.make_sure_all_receive_blocks_have_cemented_send_blocks(
                    chain.search_range(),
                    data_requester,
                )?;
            }
        }

        if self.chains_encountered == 0 {
            return Ok(BoundedCementationStep::AlreadyCemented(
                self.original_block.hash(),
            ));
        } else {
            Ok(BoundedCementationStep::Done)
        }
        //todo refresh transaction
    }

    fn restore_checkpoint_if_required<T: LedgerDataRequester>(
        &mut self,
        data_requester: &T,
    ) -> anyhow::Result<()> {
        if self.chain_stack.len() > 0 || self.is_done() {
            return Ok(()); // We still have pending chains. No checkpoint needed.
        }

        let top_hash = self
            .checkpoints
            .pop_back()
            .unwrap_or(self.original_block.hash());
        let block = self.get_block(&top_hash, data_requester)?;
        self.enqueue_for_cementation(&block, data_requester)
    }

    fn get_write_details(&self, chain: &ChainIteration) -> Option<WriteDetails> {
        let mut write_details = chain.into_write_details();
        if let Some(info) = self.confirmation_heights.get(&write_details.account) {
            if info.confirmed_height >= write_details.bottom_height {
                // our bottom is out of date
                if info.confirmed_height >= write_details.top_height {
                    // everything is already cemented
                    return None;
                }
                write_details.bottom_height = info.confirmed_height + 1;
                write_details.bottom_hash = info.first_unconfirmed.unwrap();
            }
        }
        Some(write_details)
    }

    fn cache_confirmation_height(
        &mut self,
        write: &WriteDetails,
        new_first_unconfirmed: Option<BlockHash>,
    ) {
        self.confirmation_heights.insert(
            write.account,
            ConfirmedInfo {
                confirmed_height: write.top_height,
                confirmed_frontier: write.top_hash,
                first_unconfirmed: new_first_unconfirmed,
            },
        );
    }

    fn make_sure_all_receive_blocks_have_cemented_send_blocks<T: LedgerDataRequester>(
        &mut self,
        search_range: BlockRange,
        data_requester: &mut T,
    ) -> anyhow::Result<()> {
        if let Some((receive, corresponding_send)) =
            self.find_receive_block(&search_range, data_requester)?
        {
            let current_chain = self.chain_stack.back_mut().unwrap();
            current_chain.go_to_successor_of(&receive);
            if corresponding_send.account_calculated() != receive.account_calculated() {
                self.enqueue_for_cementation(&corresponding_send, data_requester)?;
            }
        } else {
            // no more receive blocks in current chain
            self.chain_stack.back_mut().unwrap().set_done();
        }
        Ok(())
    }

    fn enqueue_for_cementation<T: LedgerDataRequester>(
        &mut self,
        block: &BlockEnum,
        data_requester: &T,
    ) -> anyhow::Result<()> {
        if let Some(lowest) = self.get_lowest_uncemented_block(&block, data_requester)? {
            // There are blocks that need to be cemented in this chain
            self.chain_stack
                .push_back(ChainIteration::new(&lowest, &block));
            self.chains_encountered += 1;
            if self.chains_encountered % self.chain_stack.max_len() == 0 {
                // Make a checkpoint every max_len() chains
                self.checkpoints.push_back(block.hash());
            }
        }
        Ok(())
    }

    fn get_lowest_uncemented_block<T: LedgerDataRequester>(
        &mut self,
        top_block: &BlockEnum,
        data_requester: &T,
    ) -> anyhow::Result<Option<BlockEnum>> {
        let account = top_block.account_calculated();
        match self.get_confirmation_height(&account, data_requester) {
            Some(info) => {
                if top_block.height() <= info.height {
                    Ok(None) // no uncemented block exists
                } else if top_block.height() - info.height == 1 {
                    Ok(Some(top_block.clone())) // top_block is the only uncemented block
                } else if top_block.height() - info.height == 2 {
                    Ok(Some(self.get_block(&top_block.previous(), data_requester)?))
                } else {
                    let frontier_block = self.get_block(&info.frontier, data_requester)?;
                    self.get_successor_block(&frontier_block, data_requester)
                }
            }
            None => Ok(Some(self.get_open_block(&account, data_requester)?)),
        }
    }

    fn get_confirmation_height<T: LedgerDataRequester>(
        &self,
        account: &Account,
        data_requester: &T,
    ) -> Option<ConfirmationHeightInfo> {
        match self.confirmation_heights.get(account) {
            Some(info) => Some(ConfirmationHeightInfo {
                height: info.confirmed_height,
                frontier: info.confirmed_frontier,
            }),
            None => data_requester.get_confirmation_height(account),
        }
    }

    fn find_receive_block<T: LedgerDataRequester>(
        &mut self,
        range: &BlockRange,
        data_requester: &mut T,
    ) -> anyhow::Result<Option<(BlockEnum, BlockEnum)>> {
        // todo check for stop flag
        let mut current = self.get_block(&range.bottom, data_requester)?;
        loop {
            if self.block_read_count > 0 && self.block_read_count % BATCH_READ_SIZE == 0 {
                // We could be traversing a very large account so we don't want to open read transactions for too long.
                data_requester.refresh_transaction();
            }
            if let Some(send) = self.get_corresponding_send_block(&current, data_requester) {
                return Ok(Some((current, send)));
            }

            if current.hash() == range.top || self.stopped.load(Ordering::Relaxed) {
                return Ok(None);
            }

            current = self
                .get_successor_block(&current, data_requester)?
                .ok_or_else(|| anyhow!("invalid block range given"))?;
        }
    }

    pub fn is_accounts_cache_full(&self) -> bool {
        self.confirmation_heights.len() >= self.chain_stack.max_len()
    }

    pub fn is_done(&self) -> bool {
        self.latest_cementation == self.original_block.hash()
            || self.stopped.load(Ordering::Relaxed)
    }

    fn get_corresponding_send_block<T: LedgerDataRequester>(
        &mut self,
        block: &BlockEnum,
        data_requester: &T,
    ) -> Option<BlockEnum> {
        let source = block.source_or_link();
        if !source.is_zero() && !self.epochs.is_epoch_link(&source.into()) {
            self.get_block(&source, data_requester).ok()
        } else {
            None
        }
    }

    pub fn clear_all_cached_accounts(&mut self) {
        self.confirmation_heights.clear();
    }

    pub fn clear_cached_account(&mut self, account: &Account, height: u64) {
        if let Some(found_info) = self.confirmation_heights.get(account) {
            if found_info.confirmed_height == height {
                self.confirmation_heights.remove(account);
            }
        }
    }

    pub(crate) fn container_info(&self) -> AccountsConfirmedMapContainerInfo {
        self.confirmation_heights.container_info()
    }

    fn get_successor_block<T: LedgerDataRequester>(
        &mut self,
        block: &BlockEnum,
        data_requester: &T,
    ) -> anyhow::Result<Option<BlockEnum>> {
        match block.successor() {
            Some(successor) => Ok(Some(self.get_block(&successor, data_requester)?)),
            None => Ok(None),
        }
    }

    fn get_open_block<T: LedgerDataRequester>(
        &mut self,
        account: &Account,
        data_requester: &T,
    ) -> anyhow::Result<BlockEnum> {
        let open_hash = data_requester
            .get_account_info(account)
            .ok_or_else(|| anyhow!("could not load account info for account {}", account))?
            .open_block;

        self.get_block(&open_hash, data_requester)
    }

    fn get_block<T: LedgerDataRequester>(
        &mut self,
        block_hash: &BlockHash,
        data_requester: &T,
    ) -> anyhow::Result<BlockEnum> {
        if *block_hash == self.original_block.hash() {
            return Ok(self.original_block.clone());
        }
        self.block_read_count += 1;
        data_requester
            .get_block(block_hash)
            .ok_or_else(|| anyhow!("could not load block {}", block_hash))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::cementing::LedgerDataRequesterStub;
    use rsnano_core::BlockChainBuilder;

    #[test]
    fn block_not_found() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut sut = BoundedModeHelperV2::builder().build();
        let genesis_chain = data_requester
            .add_genesis_block()
            .legacy_send()
            .legacy_send();
        sut.initialize(genesis_chain.latest_block().clone());
        let err = sut.get_next_step(&mut data_requester).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("could not load block {}", genesis_chain.blocks()[1].hash())
        );
    }

    #[test]
    fn stopped() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let stopped = Arc::new(AtomicBool::new(false));
        let mut sut = BoundedModeHelperV2::builder()
            .stopped(stopped.clone())
            .build();

        let genesis_chain = data_requester.add_genesis_block().legacy_send();
        data_requester.add_uncemented(&genesis_chain);
        sut.initialize(genesis_chain.latest_block().clone());

        stopped.store(true, Ordering::Relaxed);

        let step = sut.get_next_step(&mut data_requester).unwrap();
        assert_eq!(step, BoundedCementationStep::Done)
    }

    #[test]
    fn cement_first_send_from_genesis() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block().legacy_send();
        data_requester.add_uncemented(&genesis_chain);

        assert_write_steps(
            &mut data_requester,
            genesis_chain.latest_block().clone(),
            &[WriteDetails {
                account: genesis_chain.account(),
                bottom_height: 2,
                bottom_hash: genesis_chain.frontier(),
                top_height: 2,
                top_hash: genesis_chain.frontier(),
            }],
        );

        assert_eq!(data_requester.blocks_loaded(), 0);
        assert_eq!(data_requester.confirmation_heights_loaded(), 1);
    }
    #[test]
    fn cement_two_blocks_in_one_go() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester
            .add_genesis_block()
            .legacy_send()
            .legacy_send();
        let first_send = genesis_chain.blocks()[1].clone();
        let second_send = genesis_chain.blocks()[2].clone();
        data_requester.add_uncemented(&genesis_chain);

        assert_write_steps(
            &mut data_requester,
            second_send.clone(),
            &[WriteDetails {
                account: genesis_chain.account(),
                bottom_height: 2,
                bottom_hash: first_send.hash(),
                top_height: 3,
                top_hash: second_send.hash(),
            }],
        );
        assert_eq!(data_requester.blocks_loaded(), 2);
        assert_eq!(data_requester.confirmation_heights_loaded(), 1);
    }

    #[test]
    fn cement_three_blocks_in_one_go() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester
            .add_genesis_block()
            .legacy_send()
            .legacy_send()
            .legacy_send();
        data_requester.add_uncemented(&genesis_chain);

        assert_write_steps(
            &mut data_requester,
            genesis_chain.latest_block().clone(),
            &[WriteDetails {
                account: genesis_chain.account(),
                bottom_height: 2,
                bottom_hash: genesis_chain.blocks()[1].hash(),
                top_height: 4,
                top_hash: genesis_chain.frontier(),
            }],
        );
        assert_eq!(data_requester.blocks_loaded(), 4);
        assert_eq!(data_requester.confirmation_heights_loaded(), 1);
    }

    #[test]
    fn cement_open_block() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let dest_chain = BlockChainBuilder::new();
        let genesis_chain = data_requester
            .add_genesis_block()
            .legacy_send_with(|b| b.destination(dest_chain.account()));
        let dest_chain = dest_chain.legacy_open_from(genesis_chain.latest_block());
        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&dest_chain);

        assert_write_steps(
            &mut data_requester,
            dest_chain.latest_block().clone(),
            &[WriteDetails {
                account: dest_chain.account(),
                bottom_height: 1,
                bottom_hash: dest_chain.frontier(),
                top_height: 1,
                top_hash: dest_chain.frontier(),
            }],
        );
    }

    #[test]
    fn cement_open_block_and_successor_in_one_go() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block().legacy_send();
        let dest_chain =
            BlockChainBuilder::from_send_block(genesis_chain.latest_block()).legacy_send();
        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&dest_chain);

        assert_write_steps(
            &mut data_requester,
            dest_chain.latest_block().clone(),
            &[WriteDetails {
                account: dest_chain.account(),
                bottom_height: 1,
                bottom_hash: dest_chain.open(),
                top_height: 2,
                top_hash: dest_chain.frontier(),
            }],
        );
    }

    #[test]
    fn cement_open_block_and_two_successors_in_one_go() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block().legacy_send();
        let dest_chain = BlockChainBuilder::from_send_block(genesis_chain.latest_block())
            .legacy_send()
            .legacy_send();
        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&dest_chain);

        assert_write_steps(
            &mut data_requester,
            dest_chain.latest_block().clone(),
            &[WriteDetails {
                account: dest_chain.account(),
                bottom_height: 1,
                bottom_hash: dest_chain.open(),
                top_height: 3,
                top_hash: dest_chain.frontier(),
            }],
        );
    }

    #[test]
    fn cement_receive_block() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let dest_chain = BlockChainBuilder::new();
        let genesis_chain = data_requester
            .add_genesis_block()
            .legacy_send_with(|b| b.destination(dest_chain.account()))
            .legacy_send_with(|b| b.destination(dest_chain.account()));
        let dest_chain = dest_chain.legacy_open_from(&genesis_chain.blocks()[1]);
        data_requester.add_cemented(&genesis_chain);
        data_requester.add_cemented(&dest_chain);

        let dest_chain = dest_chain.legacy_receive_from(genesis_chain.latest_block());
        data_requester.add_uncemented(&dest_chain);

        assert_write_steps(
            &mut data_requester,
            dest_chain.latest_block().clone(),
            &[WriteDetails {
                account: dest_chain.account(),
                bottom_height: 2,
                bottom_hash: dest_chain.frontier(),
                top_height: 2,
                top_hash: dest_chain.frontier(),
            }],
        );
    }
    #[test]
    fn cement_two_accounts_in_one_go() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block().legacy_send();
        let dest_1 = BlockChainBuilder::from_send_block(genesis_chain.latest_block())
            .legacy_send()
            .legacy_send()
            .legacy_send_with(|b| b.destination(Account::from(7)));
        let dest_2 = BlockChainBuilder::from_send_block(dest_1.latest_block())
            .legacy_send()
            .legacy_send()
            .legacy_send();

        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&dest_1);
        data_requester.add_uncemented(&dest_2);

        assert_write_steps(
            &mut data_requester,
            dest_2.latest_block().clone(),
            &[
                WriteDetails {
                    account: dest_1.account(),
                    bottom_height: 1,
                    bottom_hash: dest_1.open(),
                    top_height: 4,
                    top_hash: dest_1.frontier(),
                },
                WriteDetails {
                    account: dest_2.account(),
                    bottom_height: 1,
                    bottom_hash: dest_2.open(),
                    top_height: 4,
                    top_hash: dest_2.frontier(),
                },
            ],
        );
    }

    #[test]
    fn send_to_self() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let chain = data_requester.add_genesis_block();
        let account = chain.account();
        let chain = chain.legacy_send_with(|b| b.destination(account));
        let send_block = chain.latest_block().clone();
        let chain = chain.legacy_receive_from(&send_block);
        data_requester.add_uncemented(&chain);

        assert_write_steps(
            &mut data_requester,
            chain.latest_block().clone(),
            &[WriteDetails {
                account: chain.account(),
                bottom_height: 2,
                bottom_hash: send_block.hash(),
                top_height: 3,
                top_hash: chain.frontier(),
            }],
        );
    }

    #[test]
    fn receive_and_send() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block().legacy_send();
        let dest_chain =
            BlockChainBuilder::from_send_block(genesis_chain.latest_block()).legacy_send();
        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&dest_chain);

        assert_write_steps(
            &mut data_requester,
            dest_chain.latest_block().clone(),
            &[WriteDetails {
                account: dest_chain.account(),
                bottom_height: 1,
                bottom_hash: dest_chain.open(),
                top_height: 2,
                top_hash: dest_chain.frontier(),
            }],
        );
    }

    #[test]
    fn complex_example() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester
            .add_genesis_block()
            .legacy_send_with(|b| b.destination(Account::from(1)));

        let account1 = BlockChainBuilder::from_send_block(genesis_chain.latest_block())
            .legacy_send()
            .legacy_send_with(|b| b.destination(Account::from(2)));

        let account2 = BlockChainBuilder::from_send_block(account1.latest_block())
            .legacy_send_with(|b| b.destination(Account::from(3)));

        let account3 = BlockChainBuilder::from_send_block(account2.latest_block())
            .legacy_send()
            .legacy_send_with(|b| b.destination(Account::from(1)));

        let account1 = account1
            .legacy_receive_from(account3.latest_block())
            .legacy_send()
            .legacy_send_with(|b| b.destination(account2.account()));

        let account2 = account2.legacy_receive_from(account1.latest_block());

        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&account1);
        data_requester.add_uncemented(&account2);
        data_requester.add_uncemented(&account3);

        assert_write_steps(
            &mut data_requester,
            account2.latest_block().clone(),
            &[
                WriteDetails {
                    account: account1.account(),
                    bottom_height: 1,
                    bottom_hash: account1.open(),
                    top_height: 3,
                    top_hash: account1.blocks()[2].hash(),
                },
                WriteDetails {
                    account: account2.account(),
                    bottom_height: 1,
                    bottom_hash: account2.open(),
                    top_height: 2,
                    top_hash: account2.blocks()[1].hash(),
                },
                WriteDetails {
                    account: account3.account(),
                    bottom_height: 1,
                    bottom_hash: account3.open(),
                    top_height: 3,
                    top_hash: account3.frontier(),
                },
                WriteDetails {
                    account: account1.account(),
                    bottom_height: 4,
                    bottom_hash: account1.blocks()[3].hash(),
                    top_height: 6,
                    top_hash: account1.frontier(),
                },
                WriteDetails {
                    account: account2.account(),
                    bottom_height: 3,
                    bottom_hash: account2.frontier(),
                    top_height: 3,
                    top_hash: account2.frontier(),
                },
            ],
        );
        assert_eq!(data_requester.blocks_loaded(), 25);
        assert_eq!(data_requester.confirmation_heights_loaded(), 5);
    }

    #[test]
    fn block_already_cemented() {
        let mut sut = BoundedModeHelperV2::builder().build();
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block();

        sut.initialize(genesis_chain.latest_block().clone());
        let step = sut.get_next_step(&mut data_requester).unwrap();

        assert_eq!(
            step,
            BoundedCementationStep::AlreadyCemented(genesis_chain.frontier())
        );
    }

    #[test]
    fn use_checkpoints() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester
            .add_genesis_block()
            .legacy_send_with(|b| b.destination(Account::from(1)));

        let account1 = BlockChainBuilder::from_send_block(genesis_chain.latest_block())
            .legacy_send_with(|b| b.destination(Account::from(2)));

        let account2 = BlockChainBuilder::from_send_block(account1.latest_block())
            .legacy_send_with(|b| b.destination(Account::from(3)));

        let account3 = BlockChainBuilder::from_send_block(account2.latest_block())
            .legacy_send_with(|b| b.destination(Account::from(4)));

        let account4 = BlockChainBuilder::from_send_block(account3.latest_block())
            .legacy_send_with(|b| b.destination(Account::from(5)));

        let account5 = BlockChainBuilder::from_send_block(account4.latest_block())
            .legacy_send_with(|b| b.destination(Account::from(6)));

        let account6 = BlockChainBuilder::from_send_block(account5.latest_block()).legacy_send();

        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&account1);
        data_requester.add_uncemented(&account2);
        data_requester.add_uncemented(&account3);
        data_requester.add_uncemented(&account4);
        data_requester.add_uncemented(&account5);
        data_requester.add_uncemented(&account6);

        assert_write_steps_with_max_items(
            2,
            &mut data_requester,
            account6.latest_block().clone(),
            &[
                WriteDetails {
                    account: account1.account(),
                    bottom_height: 1,
                    bottom_hash: account1.open(),
                    top_height: 2,
                    top_hash: account1.frontier(),
                },
                WriteDetails {
                    account: account2.account(),
                    bottom_height: 1,
                    bottom_hash: account2.open(),
                    top_height: 2,
                    top_hash: account2.frontier(),
                },
                WriteDetails {
                    account: account3.account(),
                    bottom_height: 1,
                    bottom_hash: account3.open(),
                    top_height: 2,
                    top_hash: account3.frontier(),
                },
                WriteDetails {
                    account: account4.account(),
                    bottom_height: 1,
                    bottom_hash: account4.open(),
                    top_height: 2,
                    top_hash: account4.frontier(),
                },
                WriteDetails {
                    account: account5.account(),
                    bottom_height: 1,
                    bottom_hash: account5.open(),
                    top_height: 2,
                    top_hash: account5.frontier(),
                },
                WriteDetails {
                    account: account6.account(),
                    bottom_height: 1,
                    bottom_hash: account6.open(),
                    top_height: 2,
                    top_hash: account6.frontier(),
                },
            ],
        );

        assert_eq!(data_requester.blocks_loaded(), 39);
        assert_eq!(data_requester.confirmation_heights_loaded(), 12);
    }

    mod pruning {
        use super::*;

        #[test]
        #[ignore]
        fn cement_already_pruned_block() {
            let mut sut = BoundedModeHelperV2::builder().build();
            let mut data_requester = LedgerDataRequesterStub::new();
            let hash = BlockHash::from(1);
            data_requester.prune(hash);

            // sut.initialize(&hash);
            let step = sut.get_next_step(&mut data_requester).unwrap();

            assert_eq!(step, BoundedCementationStep::Done);
        }

        #[test]
        #[ignore]
        fn send_block_pruned() {
            let mut data_requester = LedgerDataRequesterStub::new();
            let genesis_chain = data_requester.add_genesis_block().legacy_send();
            let dest_chain = BlockChainBuilder::from_send_block(genesis_chain.latest_block());
            data_requester.add_cemented(&genesis_chain);
            data_requester.add_uncemented(&dest_chain);
            data_requester.prune(genesis_chain.frontier());

            assert_write_steps(
                &mut data_requester,
                dest_chain.latest_block().clone(),
                &[WriteDetails {
                    account: dest_chain.account(),
                    bottom_height: 1,
                    bottom_hash: dest_chain.frontier(),
                    top_height: 1,
                    top_hash: dest_chain.frontier(),
                }],
            );
        }
    }

    fn assert_write_steps(
        data_requester: &mut LedgerDataRequesterStub,
        block_to_cement: BlockEnum,
        expected: &[WriteDetails],
    ) {
        assert_write_steps_with_max_items(MAX_ITEMS, data_requester, block_to_cement, expected)
    }

    fn assert_write_steps_with_max_items(
        max_items: usize,
        data_requester: &mut LedgerDataRequesterStub,
        block_to_cement: BlockEnum,
        expected: &[WriteDetails],
    ) {
        let mut sut = BoundedModeHelperV2::builder().max_items(max_items).build();
        sut.initialize(block_to_cement);

        let mut actual = Vec::new();
        loop {
            let step = sut.get_next_step(data_requester).unwrap();
            match step {
                BoundedCementationStep::Write(details) => actual.push(details),
                BoundedCementationStep::AlreadyCemented(_) => unreachable!(),
                BoundedCementationStep::Done => break,
            }
        }

        for (i, (act, exp)) in actual.iter().zip(expected).enumerate() {
            assert_eq!(act, exp, "Unexpected WriteDetails at index {}", i);
        }

        if actual.len() < expected.len() {
            panic!(
                "actual as too few elements. These are missing: {:?}",
                &expected[actual.len()..]
            );
        }

        if actual.len() > expected.len() {
            panic!(
                "actual as too many elements. These are too many: {:?}",
                &actual[expected.len()..]
            );
        }

        assert_eq!(sut.checkpoints.len(), 0);
    }
}
