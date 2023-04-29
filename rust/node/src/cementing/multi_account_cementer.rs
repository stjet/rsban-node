use std::sync::Arc;

use super::{
    batch_write_size_manager::BatchWriteSizeManagerOptions, BatchWriteSizeManager,
    CementationQueue, LedgerDataRequester, WriteBatchSlicer, WriteDetailsContainerInfo,
};
use rsnano_core::{BlockChainSection, BlockEnum};

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
pub(crate) struct MultiAccountCementer {
    /// Will contain all blocks that have been cemented (bounded by batch_write_size)
    /// and will get run through the cemented observer callback
    cemented_blocks: Vec<Arc<BlockEnum>>,
    account_cementer: WriteBatchSlicer,
    current: Option<BlockChainSection>,
    pending_writes: CementationQueue,
    pub batch_write_size: Arc<BatchWriteSizeManager>,
    max_pending_writes: usize,
}

impl Default for MultiAccountCementer {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl MultiAccountCementer {
    pub fn new(options: MultiAccountCementerOptions) -> Self {
        Self {
            cemented_blocks: Vec::new(),
            account_cementer: WriteBatchSlicer::new(Default::default(), Default::default(), 1),
            current: None,
            pending_writes: CementationQueue::new(),
            batch_write_size: Arc::new(BatchWriteSizeManager::new(BatchWriteSizeManagerOptions {
                minimum_size: options.minimum_batch_size,
            })),
            max_pending_writes: options.max_pending_writes,
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

    pub fn next_slice<T: LedgerDataRequester>(
        &mut self,
        data_requester: &T,
    ) -> anyhow::Result<Option<BlockChainSection>> {
        if self.account_cementer.is_done() {
            self.load_next_pending(data_requester);
        }

        self.account_cementer.next_batch(
            &|hash| data_requester.get_block(hash),
            &mut self.cemented_blocks,
        )
    }

    fn load_next_pending<T: LedgerDataRequester>(&mut self, data_requester: &T) {
        self.current = self.pending_writes.pop_front();
        if let Some(pending) = self.current.clone() {
            self.init_account_cementer(data_requester, pending.clone());
        }
    }

    fn init_account_cementer<T: LedgerDataRequester>(
        &mut self,
        data_requester: &T,
        pending: BlockChainSection,
    ) {
        let confirmation_height_info = data_requester
            .get_confirmation_height(&pending.account)
            .unwrap_or_default();

        self.account_cementer = WriteBatchSlicer::new(
            pending,
            confirmation_height_info,
            self.batch_write_size.current_size_with_tolerance(),
        );
    }

    pub(crate) fn is_done(&self) -> bool {
        self.account_cementer.is_done() && self.pending_writes.is_empty()
    }

    pub fn unpublished_cemented_blocks(&self) -> usize {
        self.cemented_blocks.len()
    }

    pub fn publish_cemented_blocks(&mut self, block_cemented: &mut dyn FnMut(&Arc<BlockEnum>)) {
        for block in self.cemented_blocks.drain(..) {
            block_cemented(&block);
        }
    }

    pub fn container_info(&self) -> WriteDetailsContainerInfo {
        self.pending_writes.container_info()
    }
}

#[cfg(test)]
mod tests {
    use rsnano_core::BlockChainBuilder;

    use crate::cementing::ledger_data_requester::LedgerDataRequesterStub;

    use super::*;

    #[test]
    fn empty_queue() {
        let mut slicer = MultiAccountCementer::default();
        let data_requester = LedgerDataRequesterStub::new();
        let slice = slicer.next_slice(&data_requester).unwrap();
        assert_eq!(slice, None)
    }

    #[test]
    fn one_open_block() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block().legacy_send();
        data_requester.add_cemented(&genesis_chain);
        let dest_chain = BlockChainBuilder::from_send_block(genesis_chain.latest_block());
        data_requester.add_uncemented(&dest_chain);

        let section = BlockChainSection {
            account: { dest_chain.account() },
            bottom_hash: dest_chain.open(),
            bottom_height: 1,
            top_hash: dest_chain.open(),
            top_height: 1,
        };

        let expected = [section.clone()];

        assert_slices(Default::default(), &data_requester, section, &expected);
    }

    #[test]
    fn open_block_and_successor_block() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block().legacy_send();
        data_requester.add_cemented(&genesis_chain);
        let dest_chain =
            BlockChainBuilder::from_send_block(genesis_chain.latest_block()).legacy_send();
        data_requester.add_uncemented(&dest_chain);

        let section = BlockChainSection {
            account: { dest_chain.account() },
            bottom_hash: dest_chain.open(),
            bottom_height: 1,
            top_hash: dest_chain.frontier(),
            top_height: 2,
        };

        let expected = [section.clone()];

        assert_slices(Default::default(), &data_requester, section, &expected);
    }

    #[test]
    fn skip_already_cemented_block() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block().legacy_send();
        data_requester.add_cemented(&genesis_chain);
        let genesis_chain = genesis_chain.legacy_send();
        data_requester.add_uncemented(&genesis_chain);

        let section = BlockChainSection {
            account: genesis_chain.account(),
            bottom_hash: genesis_chain.blocks()[1].hash(),
            bottom_height: 2,
            top_hash: genesis_chain.frontier(),
            top_height: 3,
        };

        let expected = [BlockChainSection {
            account: genesis_chain.account(),
            bottom_hash: genesis_chain.frontier(),
            bottom_height: 3,
            top_hash: genesis_chain.frontier(),
            top_height: 3,
        }];

        assert_slices(Default::default(), &data_requester, section, &expected);
    }

    #[test]
    fn slice_large_section() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block();
        data_requester.add_cemented(&genesis_chain);
        let genesis_chain = genesis_chain.legacy_send().legacy_send();
        data_requester.add_uncemented(&genesis_chain);

        let section = BlockChainSection {
            account: genesis_chain.account(),
            bottom_hash: genesis_chain.blocks()[1].hash(),
            bottom_height: 2,
            top_hash: genesis_chain.frontier(),
            top_height: 3,
        };

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

        assert_slices(options, &data_requester, section, &expected);
    }

    #[test]
    fn slice_large_section_and_finish_without_a_full_batch() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block();
        data_requester.add_cemented(&genesis_chain);
        let genesis_chain = genesis_chain.legacy_send().legacy_send().legacy_send();
        data_requester.add_uncemented(&genesis_chain);

        let options = MultiAccountCementerOptions {
            minimum_batch_size: 2,
            ..Default::default()
        };

        let section = BlockChainSection {
            account: genesis_chain.account(),
            bottom_hash: genesis_chain.blocks()[1].hash(),
            bottom_height: 2,
            top_hash: genesis_chain.frontier(),
            top_height: 4,
        };

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

        assert_slices(options, &data_requester, section, &expected);
    }

    fn assert_slices(
        options: MultiAccountCementerOptions,
        data_requester: &LedgerDataRequesterStub,
        section: BlockChainSection,
        expected_slices: &[BlockChainSection],
    ) {
        let mut slicer = MultiAccountCementer::new(options.clone());
        slicer.enqueue(section.clone());

        for expected in expected_slices {
            let actual = slicer.next_slice(data_requester).unwrap();
            assert_eq!(actual.as_ref(), Some(expected));
        }

        assert_eq!(slicer.next_slice(data_requester).unwrap(), None);
    }
}
