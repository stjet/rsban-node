use std::sync::Arc;

use anyhow::Context;
use rsnano_core::{BlockChainSection, BlockEnum, BlockHash, ConfirmationHeightInfo};

/// Creates ConfirmationHeightUpdates for a single account.
/// Those updates describe the changes that need to be written to
/// the confirmation height store.
pub(crate) struct WriteBatchSlicer {
    section_to_cement: BlockChainSection,
    confirmation_height_info: ConfirmationHeightInfo,
    batch_write_size: usize,
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

impl WriteBatchSlicer {
    pub fn new(
        section_to_cement: BlockChainSection,
        confirmation_height_info: ConfirmationHeightInfo,
        batch_write_size: usize,
    ) -> Self {
        Self {
            section_to_cement,
            confirmation_height_info,
            batch_write_size,
            is_initialized: false,
            num_blocks_to_cement: 0,
            total_blocks_cemented: 0,
            start_height: 0,
            next_block_index: 0,
            new_cemented_frontier_hash: Default::default(),
            new_cemented_frontier_block: None,
        }
    }

    pub fn next_batch(
        &mut self,
        load_block: &dyn Fn(&BlockHash) -> Option<BlockEnum>,
        cemented_blocks: &mut Vec<Arc<BlockEnum>>,
    ) -> anyhow::Result<Option<BlockChainSection>> {
        cemented_blocks.clear();

        if !self.is_initialized {
            self.initialize(load_block)?;
            self.is_initialized = true;
        }

        // Cementing starts from the bottom of the chain and works upwards. This is because chains can have effectively
        // an infinite number of send/change blocks in a row. We don't want to hold the write transaction open for too long.
        for i in self.next_block_index..self.num_blocks_to_cement {
            self.next_block_index = i + 1;
            let Some(new_frontier) = &self.new_cemented_frontier_block else { break; };
            cemented_blocks.push(new_frontier.clone());
            self.total_blocks_cemented += 1;

            // Flush these callbacks and continue as we write in batches (ideally maximum 250ms) to not hold write db transaction for too long.
            let slice = self.create_slice(&cemented_blocks);

            self.load_next_block_to_cement(&load_block)
                .with_context(|| {
                    format!(
                        "Could not load next block to cement for account {}",
                        self.section_to_cement.account
                    )
                })?;

            if let Some(slice) = slice {
                return Ok(Some(slice));
            }
        }

        Ok(self.create_slice(&cemented_blocks))
    }

    fn initialize(
        &mut self,
        load_block: &dyn Fn(&BlockHash) -> Option<BlockEnum>,
    ) -> Result<(), anyhow::Error> {
        let hash = self.get_first_block_to_cement(load_block)?;

        if let Some(hash) = hash {
            self.new_cemented_frontier_hash = hash;
            let new_frontier =
                Arc::new(load_block(&hash).ok_or_else(|| anyhow!("block not found"))?);

            self.start_height = new_frontier.sideband().unwrap().height;
            self.num_blocks_to_cement = self.section_to_cement.top_height - self.start_height + 1;
            self.new_cemented_frontier_block = Some(new_frontier);
        }

        Ok(())
    }

    fn get_first_block_to_cement(
        &self,
        load_block: &dyn Fn(&BlockHash) -> Option<BlockEnum>,
    ) -> anyhow::Result<Option<BlockHash>> {
        if self.are_all_blocks_cemented_already() {
            Ok(None)
        } else if self.are_some_blocks_cemented_already() {
            // We have to adjust our starting point
            let current_frontier = self.load_current_cemented_frontier(load_block)?;
            Ok(Some(current_frontier.sideband().unwrap().successor))
        } else {
            // This is the usual case where pending.bottom_height is the first uncemented block
            self.ensure_first_block_to_cement_is_one_above_current_frontier()?;
            Ok(Some(self.section_to_cement.bottom_hash))
        }
    }

    fn are_all_blocks_cemented_already(&self) -> bool {
        self.section_to_cement.top_height <= self.confirmation_height_info.height
    }

    fn are_some_blocks_cemented_already(&self) -> bool {
        self.confirmation_height_info.height >= self.section_to_cement.bottom_height
    }

    fn ensure_first_block_to_cement_is_one_above_current_frontier(&self) -> anyhow::Result<()> {
        if self.section_to_cement.bottom_height != self.confirmation_height_info.height + 1 {
            bail!("pending.bottom_height should be exactly 1 block above the cemented frontier!");
        }

        Ok(())
    }

    fn load_current_cemented_frontier(
        &self,
        load_block: &dyn Fn(&BlockHash) -> Option<BlockEnum>,
    ) -> anyhow::Result<BlockEnum> {
        load_block(&self.confirmation_height_info.frontier).ok_or_else(|| {
            anyhow!(
                "Could not load current cemented frontier {} for account {}",
                self.confirmation_height_info.frontier,
                self.section_to_cement.account
            )
        })
    }

    /// Get the next block in the chain until we have reached the final desired one
    fn load_next_block_to_cement(
        &mut self,
        load_block: &dyn Fn(&BlockHash) -> Option<BlockEnum>,
    ) -> anyhow::Result<()> {
        if !self.is_done() {
            let Some(current) = &self.new_cemented_frontier_block else { bail!("no current block loaded!") };
            self.new_cemented_frontier_hash = current.sideband().unwrap().successor;
            let next_block = load_block(&self.new_cemented_frontier_hash);
            if next_block.is_none() {
                bail!(
                    "Next block to cement not found: {}",
                    self.new_cemented_frontier_hash
                );
            }
            self.new_cemented_frontier_block = next_block.map(Arc::new);
            Ok(())
        } else if self.new_cemented_frontier_hash != self.section_to_cement.top_hash {
            // Confirm it is indeed the last one
            bail!("Last iteration reached, but top_hash does not match cemented frontier!")
        } else {
            Ok(())
        }
    }

    pub fn is_done(&self) -> bool {
        self.total_blocks_cemented == self.num_blocks_to_cement
    }

    fn create_slice(&self, cemented_blocks: &[Arc<BlockEnum>]) -> Option<BlockChainSection> {
        if self.should_flush(cemented_blocks) {
            let bottom = &cemented_blocks[0];
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

    fn should_flush(&self, cemented_blocks: &[Arc<BlockEnum>]) -> bool {
        (self.is_done() && cemented_blocks.len() > 0)
            || cemented_blocks.len() >= self.batch_write_size
    }
}
