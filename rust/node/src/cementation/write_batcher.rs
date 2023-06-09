use std::{sync::Arc, time::Duration};

use super::{
    batch_write_size_manager::BatchWriteSizeManagerOptions, BatchWriteSizeManager,
    CementationQueue, CementationQueueContainerInfo, LedgerDataRequester,
};
use rsnano_core::{BlockChainSection, BlockEnum, BlockHash, ConfirmationHeightInfo};

#[derive(Clone)]
pub(crate) struct WriteBatcherOptions {
    pub max_pending_writes: usize,
    pub min_batch_size: usize,
}

impl WriteBatcherOptions {
    pub const DEFAULT_MAX_PENDING_WRITES: usize = 0x20000;
}

impl Default for WriteBatcherOptions {
    fn default() -> Self {
        Self {
            max_pending_writes: Self::DEFAULT_MAX_PENDING_WRITES,
            min_batch_size: BatchWriteSizeManagerOptions::DEFAULT_MIN_SIZE,
        }
    }
}

/// Writes all confirmation heights from the WriteDetailsQueue to the Ledger.
/// This happens in batches in order to increase performance.
pub(crate) struct WriteBatcher {
    /// Will contain all blocks that have been cemented (bounded by batch_write_size)
    /// and will get run through the cemented observer callback
    cemented_blocks: Vec<Arc<BlockEnum>>,
    current: Option<BlockChainSection>,
    pending_writes: CementationQueue,
    pub batch_write_size: Arc<BatchWriteSizeManager>,
    max_pending_writes: usize,

    section_to_cement: BlockChainSection,
    confirmation_height_info: ConfirmationHeightInfo,
    is_initialized: bool,
    /// The total number of blocks to cement
    num_blocks_to_cement: u64,
    total_blocks_cemented_for_current_account: u64,
    /// The block height of the first block to cement
    start_height: u64,
    next_block_index: u64,
    new_cemented_frontier_hash: BlockHash,
    new_cemented_frontier_block: Option<Arc<BlockEnum>>,
    bottom_hash: BlockHash,
    bottom_height: u64,
}

impl Default for WriteBatcher {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl WriteBatcher {
    pub fn new(options: WriteBatcherOptions) -> Self {
        Self {
            cemented_blocks: Vec::new(),
            current: None,
            pending_writes: CementationQueue::new(),
            batch_write_size: Arc::new(BatchWriteSizeManager::new(BatchWriteSizeManagerOptions {
                min_size: options.min_batch_size,
            })),
            max_pending_writes: options.max_pending_writes,

            section_to_cement: Default::default(),
            confirmation_height_info: Default::default(),
            is_initialized: false,
            num_blocks_to_cement: 0,
            total_blocks_cemented_for_current_account: 0,
            start_height: 0,
            next_block_index: 0,
            new_cemented_frontier_hash: Default::default(),
            new_cemented_frontier_block: None,
            bottom_hash: BlockHash::zero(),
            bottom_height: 0,
        }
    }

    pub fn max_batch_size_reached(&self) -> bool {
        self.pending_writes.total_pending_blocks() >= self.batch_write_size.current_size()
    }

    pub fn max_pending_writes_reached(&self) -> bool {
        self.pending_writes.len() >= self.max_pending_writes
    }

    pub fn has_pending_writes(&self) -> bool {
        self.pending_writes.len() > 0
    }

    pub fn enqueue(&mut self, write_details: BlockChainSection) {
        self.pending_writes.push_back(write_details);
    }

    pub fn next_write<T: LedgerDataRequester>(
        &mut self,
        data_requester: &mut T,
    ) -> Option<BlockChainSection> {
        if self.is_current_account_done() {
            self.load_next_pending(data_requester);
        }

        self.next_batch(&mut |hash| data_requester.get_block(hash))
    }

    fn load_next_pending<T: LedgerDataRequester>(&mut self, data_requester: &T) {
        self.current = self.pending_writes.pop_front();
        if let Some(pending) = &self.current {
            self.init_account(data_requester, pending.clone());
        }
    }

    fn init_account<T: LedgerDataRequester>(
        &mut self,
        data_requester: &T,
        pending: BlockChainSection,
    ) {
        let confirmation_height_info = data_requester
            .get_confirmation_height(&pending.account)
            .unwrap_or_default();

        self.section_to_cement = pending;
        self.confirmation_height_info = confirmation_height_info;
        self.is_initialized = false;
        self.num_blocks_to_cement = 0;
        self.total_blocks_cemented_for_current_account = 0;
        self.start_height = 0;
        self.next_block_index = 0;
        self.new_cemented_frontier_hash = Default::default();
        self.new_cemented_frontier_block = None;
    }

    pub(crate) fn is_done(&self) -> bool {
        self.is_current_account_done() && self.pending_writes.is_empty()
    }

    pub fn batch_completed(
        &mut self,
        time_spent_cementing: Duration,
        block_cemented: &mut dyn FnMut(&Arc<BlockEnum>),
    ) {
        self.batch_write_size
            .adjust_size(time_spent_cementing, self.cemented_blocks.len());
        for block in self.cemented_blocks.drain(..) {
            block_cemented(&block);
        }
    }

    pub fn container_info(&self) -> CementationQueueContainerInfo {
        self.pending_writes.container_info()
    }

    fn next_batch(
        &mut self,
        load_block: &mut dyn FnMut(&BlockHash) -> Option<BlockEnum>,
    ) -> Option<BlockChainSection> {
        if !self.is_initialized {
            self.initialize(load_block);
            self.is_initialized = true;
        }

        // Cementing starts from the bottom of the chain and works upwards. This is because chains can have effectively
        // an infinite number of send/change blocks in a row. We don't want to hold the write transaction open for too long.
        for i in self.next_block_index..self.num_blocks_to_cement {
            self.next_block_index = i + 1;
            let Some(new_frontier) = &self.new_cemented_frontier_block else { break; };
            self.cemented_blocks.push(new_frontier.clone());
            if self.bottom_height == 0 {
                self.bottom_height = new_frontier.height();
                self.bottom_hash = new_frontier.hash();
            }
            self.total_blocks_cemented_for_current_account += 1;

            // Flush these callbacks and continue as we write in batches (ideally maximum 250ms) to not hold write db transaction for too long.
            let slice = self.create_slice();

            self.load_next_block_to_cement(load_block);

            if let Some(slice) = slice {
                return Some(slice);
            }
        }

        self.create_slice()
    }

    fn initialize(&mut self, load_block: &mut dyn FnMut(&BlockHash) -> Option<BlockEnum>) {
        let hash = self.get_first_block_to_cement(load_block);

        if let Some(hash) = hash {
            self.new_cemented_frontier_hash = hash;
            let new_frontier = Arc::new(load_block(&hash).expect("block not found"));

            self.start_height = new_frontier.sideband().unwrap().height;
            self.num_blocks_to_cement = self.section_to_cement.top_height - self.start_height + 1;
            self.new_cemented_frontier_block = Some(new_frontier);
        }
    }

    fn get_first_block_to_cement(
        &self,
        load_block: &mut dyn FnMut(&BlockHash) -> Option<BlockEnum>,
    ) -> Option<BlockHash> {
        if self.are_all_blocks_cemented_already() {
            None
        } else if self.are_some_blocks_cemented_already() {
            // We have to adjust our starting point
            let current_frontier = self.load_current_cemented_frontier(load_block);
            Some(current_frontier.sideband().unwrap().successor)
        } else {
            // This is the usual case where pending.bottom_height is the first uncemented block
            self.ensure_first_block_to_cement_is_one_above_current_frontier();
            Some(self.section_to_cement.bottom_hash)
        }
    }

    fn are_all_blocks_cemented_already(&self) -> bool {
        self.section_to_cement.top_height <= self.confirmation_height_info.height
    }

    fn are_some_blocks_cemented_already(&self) -> bool {
        self.confirmation_height_info.height >= self.section_to_cement.bottom_height
    }

    fn ensure_first_block_to_cement_is_one_above_current_frontier(&self) {
        if self.section_to_cement.bottom_height != self.confirmation_height_info.height + 1 {
            panic!("pending.bottom_height should be exactly 1 block above the cemented frontier!");
        }
    }

    fn load_current_cemented_frontier(
        &self,
        load_block: &mut dyn FnMut(&BlockHash) -> Option<BlockEnum>,
    ) -> BlockEnum {
        let Some(block) = load_block(&self.confirmation_height_info.frontier) else {
            panic!(
                "Could not load current cemented frontier {} for account {}",
                self.confirmation_height_info.frontier,
                self.section_to_cement.account
            )
        };
        block
    }

    /// Get the next block in the chain until we have reached the final desired one
    fn load_next_block_to_cement(
        &mut self,
        load_block: &mut dyn FnMut(&BlockHash) -> Option<BlockEnum>,
    ) {
        if !self.is_current_account_done() {
            let Some(current) = &self.new_cemented_frontier_block else { panic!("no current block loaded!") };
            self.new_cemented_frontier_hash = current.sideband().unwrap().successor;
            let next_block = load_block(&self.new_cemented_frontier_hash);
            if next_block.is_none() {
                panic!(
                    "Next block to cement not found: {} for account {}",
                    self.new_cemented_frontier_hash,
                    current.account_calculated().encode_account()
                );
            }
            self.new_cemented_frontier_block = next_block.map(Arc::new);
        } else if self.new_cemented_frontier_hash != self.section_to_cement.top_hash {
            // Confirm it is indeed the last one
            panic!("Last iteration reached, but top_hash does not match cemented frontier!")
        } else {
        }
    }

    pub fn is_current_account_done(&self) -> bool {
        self.total_blocks_cemented_for_current_account == self.num_blocks_to_cement
    }

    fn create_slice(&mut self) -> Option<BlockChainSection> {
        if self.should_create_slice() {
            let section = BlockChainSection {
                account: self.section_to_cement.account,
                top_hash: self.new_cemented_frontier_hash,
                top_height: self.start_height + self.total_blocks_cemented_for_current_account - 1,
                bottom_hash: self.bottom_hash,
                bottom_height: self.bottom_height,
            };
            self.bottom_hash = BlockHash::zero();
            self.bottom_height = 0;
            Some(section)
        } else {
            None
        }
    }

    fn should_create_slice(&self) -> bool {
        self.bottom_height > 0
            && (self.is_current_account_done()
                || self.cemented_blocks.len()
                    >= self.batch_write_size.current_size_with_tolerance())
    }

    pub fn unpublished_cemented_blocks_len(&self) -> usize {
        self.cemented_blocks.len()
    }

    pub fn should_start_new_batch(&self) -> bool {
        self.cemented_blocks.len() >= self.batch_write_size.current_size_with_tolerance()
            && !self.is_done()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cementation::LedgerDataRequesterStub;
    use rsnano_core::{Amount, TestAccountChain};

    #[test]
    fn empty_queue() {
        let mut write_batcher = WriteBatcher::default();
        let mut data_requester = LedgerDataRequesterStub::new();
        let write = write_batcher.next_write(&mut data_requester);
        assert_eq!(write, None)
    }

    #[test]
    fn one_open_block() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut dest_chain = TestAccountChain::new();
        let mut genesis_chain = data_requester.add_genesis_block();
        genesis_chain.add_legacy_send_to(dest_chain.account(), Amount::raw(1));
        data_requester.add_cemented(&genesis_chain);
        dest_chain.add_legacy_open_from_account(&genesis_chain);
        data_requester.add_uncemented(&dest_chain);

        let sections = [dest_chain.section(1, 1)];
        let expected = sections.clone();
        assert_writes(
            Default::default(),
            &mut data_requester,
            &sections,
            &expected,
        );
    }

    #[test]
    fn open_block_and_successor_block() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut dest_chain = TestAccountChain::new();
        let mut genesis_chain = data_requester.add_genesis_block();
        genesis_chain.add_legacy_send_to(dest_chain.account(), Amount::raw(1));
        data_requester.add_cemented(&genesis_chain);
        dest_chain.add_legacy_open_from_account(&genesis_chain);
        dest_chain.add_legacy_send();
        data_requester.add_uncemented(&dest_chain);

        let sections = [dest_chain.section(1, 2)];
        let expected = sections.clone();
        assert_writes(
            Default::default(),
            &mut data_requester,
            &sections,
            &expected,
        );
    }

    #[test]
    fn skip_already_cemented_block() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut genesis_chain = data_requester.add_genesis_block();
        genesis_chain.add_legacy_send();
        data_requester.add_cemented(&genesis_chain);
        genesis_chain.add_legacy_send();
        data_requester.add_uncemented(&genesis_chain);

        let sections = [genesis_chain.section(2, 3)];
        let expected = [genesis_chain.section(3, 3)];

        assert_writes(
            Default::default(),
            &mut data_requester,
            &sections,
            &expected,
        );
    }

    #[test]
    fn slice_large_section() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut genesis_chain = data_requester.add_genesis_block();
        data_requester.add_cemented(&genesis_chain);
        genesis_chain.add_legacy_send();
        genesis_chain.add_legacy_send();
        data_requester.add_uncemented(&genesis_chain);

        let sections = [genesis_chain.section(2, 3)];

        let options = WriteBatcherOptions {
            min_batch_size: 1,
            ..Default::default()
        };

        let expected = [genesis_chain.section(2, 2), genesis_chain.section(3, 3)];

        assert_writes(options, &mut data_requester, &sections, &expected);
    }

    #[test]
    fn slice_large_section_and_finish_without_a_full_batch() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut genesis_chain = data_requester.add_genesis_block();
        genesis_chain.add_legacy_send();
        genesis_chain.add_legacy_send();
        genesis_chain.add_legacy_send();
        data_requester.add_uncemented(&genesis_chain);

        let options = WriteBatcherOptions {
            min_batch_size: 2,
            ..Default::default()
        };

        let sections = [genesis_chain.section(2, 4)];
        let expected = [genesis_chain.section(2, 3), genesis_chain.section(4, 4)];

        assert_writes(options, &mut data_requester, &sections, &expected);
    }

    #[test]
    fn enqueue_two_accounts() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut dest_chain = TestAccountChain::new();
        let mut genesis_chain = data_requester.add_genesis_block();
        genesis_chain.add_legacy_send_to(dest_chain.account(), Amount::raw(1));
        data_requester.add_uncemented(&genesis_chain);
        dest_chain.add_legacy_open_from_account(&genesis_chain);
        data_requester.add_uncemented(&dest_chain);

        let sections = [genesis_chain.section(2, 2), dest_chain.section(1, 1)];

        assert_writes(
            Default::default(),
            &mut data_requester,
            &sections,
            &sections,
        );
    }

    fn assert_writes(
        options: WriteBatcherOptions,
        data_requester: &mut LedgerDataRequesterStub,
        sections: &[BlockChainSection],
        expected_slices: &[BlockChainSection],
    ) {
        let mut write_batcher = WriteBatcher::new(options.clone());
        for section in sections {
            write_batcher.enqueue(section.clone());
        }

        for (i, expected) in expected_slices.iter().enumerate() {
            let actual = write_batcher.next_write(data_requester);
            assert_eq!(actual.as_ref(), Some(expected), "at index {}", i);
        }

        assert_eq!(write_batcher.next_write(data_requester), None);
    }
}
