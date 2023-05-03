use std::sync::Arc;

use super::{
    batch_write_size_manager::BatchWriteSizeManagerOptions, BatchWriteSizeManager,
    CementationQueue, CementationQueueContainerInfo, LedgerDataRequester,
};
use rsnano_core::{BlockChainSection, BlockEnum, BlockHash, ConfirmationHeightInfo};

#[derive(Clone)]
pub(crate) struct MultiAccountCementerOptions {
    pub max_pending_writes: usize,
    pub minimum_batch_size: usize,
}

impl Default for MultiAccountCementerOptions {
    fn default() -> Self {
        let batch_size = BatchWriteSizeManagerOptions::default();
        Self {
            max_pending_writes: 0x20000,
            minimum_batch_size: batch_size.minimum_size,
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
    total_blocks_cemented: u64,
    /// The block height of the first block to cement
    start_height: u64,
    next_block_index: u64,
    new_cemented_frontier_hash: BlockHash,
    new_cemented_frontier_block: Option<Arc<BlockEnum>>,
}

impl Default for WriteBatcher {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl WriteBatcher {
    pub fn new(options: MultiAccountCementerOptions) -> Self {
        Self {
            cemented_blocks: Vec::new(),
            current: None,
            pending_writes: CementationQueue::new(),
            batch_write_size: Arc::new(BatchWriteSizeManager::new(BatchWriteSizeManagerOptions {
                minimum_size: options.minimum_batch_size,
            })),
            max_pending_writes: options.max_pending_writes,

            section_to_cement: Default::default(),
            confirmation_height_info: Default::default(),
            is_initialized: false,
            num_blocks_to_cement: 0,
            total_blocks_cemented: 0,
            start_height: 0,
            next_block_index: 0,
            new_cemented_frontier_hash: Default::default(),
            new_cemented_frontier_block: None,
        }
    }

    pub fn max_batch_write_size_reached(&self) -> bool {
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
        data_requester: &T,
    ) -> Option<BlockChainSection> {
        if self.is_current_account_done() {
            self.load_next_pending(data_requester);
        }

        self.next_batch(&|hash| data_requester.get_block(hash))
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
        self.total_blocks_cemented = 0;
        self.start_height = 0;
        self.next_block_index = 0;
        self.new_cemented_frontier_hash = Default::default();
        self.new_cemented_frontier_block = None;
    }

    pub(crate) fn is_done(&self) -> bool {
        self.is_current_account_done() && self.pending_writes.is_empty()
    }

    pub fn unpublished_cemented_blocks(&self) -> usize {
        self.cemented_blocks.len()
    }

    pub fn publish_cemented_blocks(&mut self, block_cemented: &mut dyn FnMut(&Arc<BlockEnum>)) {
        for block in self.cemented_blocks.drain(..) {
            block_cemented(&block);
        }
    }

    pub fn container_info(&self) -> CementationQueueContainerInfo {
        self.pending_writes.container_info()
    }

    fn next_batch(
        &mut self,
        load_block: &dyn Fn(&BlockHash) -> Option<BlockEnum>,
    ) -> Option<BlockChainSection> {
        self.cemented_blocks.clear();

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
            self.total_blocks_cemented += 1;

            // Flush these callbacks and continue as we write in batches (ideally maximum 250ms) to not hold write db transaction for too long.
            let slice = self.create_slice();

            self.load_next_block_to_cement(&load_block);

            if let Some(slice) = slice {
                return Some(slice);
            }
        }

        self.create_slice()
    }

    fn initialize(&mut self, load_block: &dyn Fn(&BlockHash) -> Option<BlockEnum>) {
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
        load_block: &dyn Fn(&BlockHash) -> Option<BlockEnum>,
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
        load_block: &dyn Fn(&BlockHash) -> Option<BlockEnum>,
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
    fn load_next_block_to_cement(&mut self, load_block: &dyn Fn(&BlockHash) -> Option<BlockEnum>) {
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
        self.total_blocks_cemented == self.num_blocks_to_cement
    }

    fn create_slice(&self) -> Option<BlockChainSection> {
        if self.should_flush() {
            let bottom = &self.cemented_blocks[0];
            Some(BlockChainSection {
                account: self.section_to_cement.account,
                top_hash: self.new_cemented_frontier_hash,
                top_height: self.start_height + self.total_blocks_cemented - 1,
                bottom_hash: bottom.hash(),
                bottom_height: bottom.height(),
            })
        } else {
            None
        }
    }

    fn should_flush(&self) -> bool {
        (self.is_current_account_done() && self.cemented_blocks.len() > 0)
            || self.cemented_blocks.len() >= self.batch_write_size.current_size_with_tolerance()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cementing::LedgerDataRequesterStub;
    use rsnano_core::BlockChainBuilder;

    #[test]
    fn empty_queue() {
        let mut write_batcher = WriteBatcher::default();
        let data_requester = LedgerDataRequesterStub::new();
        let write = write_batcher.next_write(&data_requester);
        assert_eq!(write, None)
    }

    #[test]
    fn one_open_block() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block().legacy_send();
        data_requester.add_cemented(&genesis_chain);
        let dest_chain = BlockChainBuilder::from_send_block(genesis_chain.latest_block());
        data_requester.add_uncemented(&dest_chain);

        let sections = [BlockChainSection {
            account: { dest_chain.account() },
            bottom_hash: dest_chain.open(),
            bottom_height: 1,
            top_hash: dest_chain.open(),
            top_height: 1,
        }];

        let expected = sections.clone();

        assert_writes(Default::default(), &data_requester, &sections, &expected);
    }

    #[test]
    fn open_block_and_successor_block() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block().legacy_send();
        data_requester.add_cemented(&genesis_chain);
        let dest_chain =
            BlockChainBuilder::from_send_block(genesis_chain.latest_block()).legacy_send();
        data_requester.add_uncemented(&dest_chain);

        let sections = [BlockChainSection {
            account: { dest_chain.account() },
            bottom_hash: dest_chain.open(),
            bottom_height: 1,
            top_hash: dest_chain.frontier(),
            top_height: 2,
        }];

        let expected = sections.clone();

        assert_writes(Default::default(), &data_requester, &sections, &expected);
    }

    #[test]
    fn skip_already_cemented_block() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block().legacy_send();
        data_requester.add_cemented(&genesis_chain);
        let genesis_chain = genesis_chain.legacy_send();
        data_requester.add_uncemented(&genesis_chain);

        let sections = [BlockChainSection {
            account: genesis_chain.account(),
            bottom_hash: genesis_chain.blocks()[1].hash(),
            bottom_height: 2,
            top_hash: genesis_chain.frontier(),
            top_height: 3,
        }];

        let expected = [BlockChainSection {
            account: genesis_chain.account(),
            bottom_hash: genesis_chain.frontier(),
            bottom_height: 3,
            top_hash: genesis_chain.frontier(),
            top_height: 3,
        }];

        assert_writes(Default::default(), &data_requester, &sections, &expected);
    }

    #[test]
    fn slice_large_section() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block();
        data_requester.add_cemented(&genesis_chain);
        let genesis_chain = genesis_chain.legacy_send().legacy_send();
        data_requester.add_uncemented(&genesis_chain);

        let sections = [BlockChainSection {
            account: genesis_chain.account(),
            bottom_hash: genesis_chain.blocks()[1].hash(),
            bottom_height: 2,
            top_hash: genesis_chain.frontier(),
            top_height: 3,
        }];

        let options = MultiAccountCementerOptions {
            minimum_batch_size: 1,
            ..Default::default()
        };

        let expected = [
            BlockChainSection {
                account: genesis_chain.account(),
                bottom_hash: genesis_chain.blocks()[1].hash(),
                bottom_height: 2,
                top_hash: genesis_chain.blocks()[1].hash(),
                top_height: 2,
            },
            BlockChainSection {
                account: genesis_chain.account(),
                bottom_hash: genesis_chain.frontier(),
                bottom_height: 3,
                top_hash: genesis_chain.frontier(),
                top_height: 3,
            },
        ];

        assert_writes(options, &data_requester, &sections, &expected);
    }

    #[test]
    fn slice_large_section_and_finish_without_a_full_batch() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block();
        let genesis_chain = genesis_chain.legacy_send().legacy_send().legacy_send();
        data_requester.add_uncemented(&genesis_chain);

        let options = MultiAccountCementerOptions {
            minimum_batch_size: 2,
            ..Default::default()
        };

        let sections = [BlockChainSection {
            account: genesis_chain.account(),
            bottom_hash: genesis_chain.blocks()[1].hash(),
            bottom_height: 2,
            top_hash: genesis_chain.frontier(),
            top_height: 4,
        }];

        let expected = [
            BlockChainSection {
                account: genesis_chain.account(),
                bottom_hash: genesis_chain.blocks()[1].hash(),
                bottom_height: 2,
                top_hash: genesis_chain.blocks()[2].hash(),
                top_height: 3,
            },
            BlockChainSection {
                account: genesis_chain.account(),
                bottom_hash: genesis_chain.frontier(),
                bottom_height: 4,
                top_hash: genesis_chain.frontier(),
                top_height: 4,
            },
        ];

        assert_writes(options, &data_requester, &sections, &expected);
    }

    #[test]
    fn enqueue_two_accounts() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block().legacy_send();
        data_requester.add_uncemented(&genesis_chain);
        let dest_chain = BlockChainBuilder::from_send_block(genesis_chain.latest_block());
        data_requester.add_uncemented(&dest_chain);

        let sections = [
            BlockChainSection {
                account: genesis_chain.account(),
                bottom_hash: genesis_chain.frontier(),
                bottom_height: 2,
                top_hash: genesis_chain.frontier(),
                top_height: 2,
            },
            BlockChainSection {
                account: dest_chain.account(),
                bottom_hash: dest_chain.frontier(),
                bottom_height: 1,
                top_hash: dest_chain.frontier(),
                top_height: 1,
            },
        ];

        assert_writes(Default::default(), &data_requester, &sections, &sections);
    }

    fn assert_writes(
        options: MultiAccountCementerOptions,
        data_requester: &LedgerDataRequesterStub,
        sections: &[BlockChainSection],
        expected_slices: &[BlockChainSection],
    ) {
        let mut write_batcher = WriteBatcher::new(options.clone());
        for section in sections {
            write_batcher.enqueue(section.clone());
        }

        for expected in expected_slices {
            let actual = write_batcher.next_write(data_requester);
            assert_eq!(actual.as_ref(), Some(expected));
        }

        assert_eq!(write_batcher.next_write(data_requester), None);
    }
}