use std::sync::Arc;

use anyhow::Context;
use rsnano_core::{BlockEnum, BlockHash, ConfirmationHeightInfo, ConfirmationHeightUpdate};

use super::write_details_queue::WriteDetails;

/// Creates ConfirmationHeightUpdates for a single account.
/// Those updates describe the changes that need to be written to
/// the confirmation height store.
#[derive(Default)]
pub(crate) struct SingleAccountCementer {
    pending: WriteDetails,
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

impl SingleAccountCementer {
    pub fn new(
        pending: WriteDetails,
        confirmation_height_info: ConfirmationHeightInfo,
        batch_write_size: usize,
    ) -> Self {
        Self {
            pending,
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

    pub fn cement(
        &mut self,
        load_block: &dyn Fn(&BlockHash) -> Option<BlockEnum>,
        cemented_blocks: &mut Vec<Arc<BlockEnum>>,
    ) -> anyhow::Result<Option<(ConfirmationHeightUpdate, bool)>> {
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
            let update_command = self.get_update_command(&cemented_blocks);

            self.load_next_block_to_cement(&load_block)
                .with_context(|| {
                    format!(
                        "Could not load next block to cement for account {}",
                        self.pending.account
                    )
                })?;

            if let Some(cmd) = update_command {
                return Ok(Some((cmd, self.is_done())));
            }
        }

        Ok(self
            .get_update_command(&cemented_blocks)
            .map(|cmd| (cmd, self.is_done())))
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
            self.num_blocks_to_cement = self.pending.top_height - self.start_height + 1;
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
            Ok(Some(self.pending.bottom_hash))
        }
    }

    fn are_all_blocks_cemented_already(&self) -> bool {
        self.pending.top_height <= self.confirmation_height_info.height
    }

    fn are_some_blocks_cemented_already(&self) -> bool {
        self.confirmation_height_info.height >= self.pending.bottom_height
    }

    fn ensure_first_block_to_cement_is_one_above_current_frontier(&self) -> anyhow::Result<()> {
        if self.pending.bottom_height != self.confirmation_height_info.height + 1 {
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
                self.pending.account
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
        } else if self.new_cemented_frontier_hash != self.pending.top_hash {
            // Confirm it is indeed the last one
            bail!("Last iteration reached, but top_hash does not match cemented frontier!")
        } else {
            Ok(())
        }
    }

    pub fn is_done(&self) -> bool {
        self.total_blocks_cemented == self.num_blocks_to_cement
    }

    fn get_update_command(
        &self,
        cemented_blocks: &[Arc<BlockEnum>],
    ) -> Option<ConfirmationHeightUpdate> {
        if self.should_flush(cemented_blocks) {
            Some(ConfirmationHeightUpdate {
                account: self.pending.account,
                new_cemented_frontier: self.new_cemented_frontier_hash,
                new_height: self.start_height + self.total_blocks_cemented - 1,
                num_blocks_cemented: cemented_blocks.len() as u64,
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

#[cfg(test)]
mod tests {
    use rsnano_core::{Account, Amount, BlockBuilder, BlockDetails, BlockSideband, Epoch};
    use std::{collections::HashMap, ops::Deref};

    use super::*;

    #[test]
    fn doesnt_cement_empty_write_details() {
        let pending = WriteDetails {
            account: Account::from(1),
            bottom_height: 0,
            bottom_hash: BlockHash::zero(),
            top_height: 0,
            top_hash: BlockHash::zero(),
        };

        let conf_height = ConfirmationHeightInfo::default();

        let mut command_factory = SingleAccountCementer::new(pending, conf_height, 42);

        let mut cemented_blocks = Vec::new();
        let command = command_factory
            .cement(&|_| unimplemented!(), &mut cemented_blocks)
            .unwrap();

        assert_eq!(command, None);
    }

    #[test]
    fn one_block_already_cemented() {
        let pending = WriteDetails {
            account: Account::from(1),
            bottom_height: 3,
            bottom_hash: BlockHash::from(7),
            top_height: 3,
            top_hash: BlockHash::from(7),
        };

        let conf_height = ConfirmationHeightInfo {
            height: 3,
            frontier: BlockHash::from(7),
        };

        let mut command_factory = SingleAccountCementer::new(pending, conf_height, 42);

        let mut cemented_blocks = Vec::new();
        let command = command_factory
            .cement(&|_| unimplemented!(), &mut cemented_blocks)
            .unwrap();

        assert_eq!(command, None);
        assert_eq!(command_factory.is_done(), true);
    }

    #[test]
    fn cement_first_block_of_account() {
        let blocks = AccountBlocksBuilder::for_account(1).add_block().build();

        let pending = WriteDetails {
            account: Account::from(1),
            bottom_height: 1,
            bottom_hash: blocks[0].hash(),
            top_height: 1,
            top_hash: blocks[0].hash(),
        };

        let conf_height = ConfirmationHeightInfo::default();

        let mut command_factory = SingleAccountCementer::new(pending, conf_height, 42);

        let load_block = create_block_loader(&blocks);
        let mut cemented_blocks = Vec::new();
        let command = command_factory
            .cement(&load_block, &mut cemented_blocks)
            .unwrap()
            .expect("command was None!");

        assert_eq!(
            command,
            (
                ConfirmationHeightUpdate {
                    account: Account::from(1),
                    new_cemented_frontier: blocks[0].hash(),
                    new_height: 1,
                    num_blocks_cemented: 1
                },
                true
            )
        );
        assert_blocks_equal(&cemented_blocks, &blocks);
        assert!(command_factory.is_done());
    }

    #[test]
    fn cement_first_two_blocks_of_account() {
        let blocks = AccountBlocksBuilder::for_account(42)
            .add_block()
            .add_block()
            .build();

        let pending = WriteDetails {
            account: Account::from(42),
            bottom_height: 1,
            bottom_hash: blocks[0].hash(),
            top_height: 2,
            top_hash: blocks[1].hash(),
        };

        let conf_height = ConfirmationHeightInfo::default();

        let mut command_factory = SingleAccountCementer::new(pending, conf_height, 42);

        let load_block = create_block_loader(&blocks);
        let mut cemented_blocks = Vec::new();
        let command = command_factory
            .cement(&load_block, &mut cemented_blocks)
            .unwrap()
            .expect("command was None!");

        assert_eq!(
            command,
            (
                ConfirmationHeightUpdate {
                    account: Account::from(42),
                    new_cemented_frontier: blocks[1].hash(),
                    new_height: 2,
                    num_blocks_cemented: 2
                },
                true
            )
        );
        assert_blocks_equal(&cemented_blocks, &blocks);
        assert!(command_factory.is_done());
    }

    #[test]
    fn skip_already_cemented_blocks() {
        let blocks = AccountBlocksBuilder::for_account(42)
            .add_block()
            .add_block()
            .build();

        let pending = WriteDetails {
            account: Account::from(42),
            bottom_height: 1,
            bottom_hash: blocks[0].hash(),
            top_height: 2,
            top_hash: blocks[1].hash(),
        };

        let conf_height = ConfirmationHeightInfo {
            height: 1,
            frontier: pending.bottom_hash,
        };

        let mut command_factory = SingleAccountCementer::new(pending, conf_height, 42);

        let load_block = create_block_loader(&blocks);
        let mut cemented_blocks = Vec::new();
        let command = command_factory
            .cement(&load_block, &mut cemented_blocks)
            .unwrap()
            .expect("command was None!");

        assert_eq!(
            command,
            (
                ConfirmationHeightUpdate {
                    account: Account::from(42),
                    new_cemented_frontier: blocks[1].hash(),
                    new_height: 2,
                    num_blocks_cemented: 1
                },
                true
            )
        );
        assert_blocks_equal(&cemented_blocks, &blocks[1..]);
        assert!(command_factory.is_done());
    }

    #[test]
    fn create_update_commands_in_batches() {
        let blocks = AccountBlocksBuilder::for_account(42)
            .add_block()
            .add_block()
            .build();

        let pending = WriteDetails {
            account: Account::from(42),
            bottom_height: 1,
            bottom_hash: blocks[0].hash(),
            top_height: 2,
            top_hash: blocks[1].hash(),
        };

        let conf_height = ConfirmationHeightInfo::default();

        let mut command_factory = SingleAccountCementer::new(pending, conf_height, 1);

        let load_block = create_block_loader(&blocks);
        let mut cemented_blocks = Vec::new();

        // Cement first batch
        let command = command_factory
            .cement(&load_block, &mut cemented_blocks)
            .unwrap()
            .expect("command was None!");

        assert_eq!(
            command,
            (
                ConfirmationHeightUpdate {
                    account: Account::from(42),
                    new_cemented_frontier: blocks[0].hash(),
                    new_height: 1,
                    num_blocks_cemented: 1
                },
                false
            )
        );
        assert_blocks_equal(&cemented_blocks, &blocks[0..1]);
        assert_eq!(command_factory.is_done(), false);

        // Cement second batch
        let command = command_factory
            .cement(&load_block, &mut cemented_blocks)
            .unwrap()
            .unwrap();

        assert_eq!(
            command,
            (
                ConfirmationHeightUpdate {
                    account: Account::from(42),
                    new_cemented_frontier: blocks[1].hash(),
                    new_height: 2,
                    num_blocks_cemented: 1
                },
                true
            )
        );
        assert_blocks_equal(&cemented_blocks, &blocks[1..]);
        assert!(command_factory.is_done());
    }

    #[test]
    fn create_update_commands_in_batches_and_finish_without_a_full_batch() {
        let blocks = AccountBlocksBuilder::for_account(42)
            .add_block()
            .add_block()
            .add_block()
            .build();

        let pending = WriteDetails {
            account: Account::from(42),
            bottom_height: 1,
            bottom_hash: blocks[0].hash(),
            top_height: 3,
            top_hash: blocks[2].hash(),
        };

        let conf_height = ConfirmationHeightInfo::default();

        let mut command_factory = SingleAccountCementer::new(pending, conf_height, 2);

        let load_block = create_block_loader(&blocks);
        let mut cemented_blocks = Vec::new();

        // Cement first batch
        let command = command_factory
            .cement(&load_block, &mut cemented_blocks)
            .unwrap()
            .expect("command was None!");

        assert_eq!(
            command,
            (
                ConfirmationHeightUpdate {
                    account: Account::from(42),
                    new_cemented_frontier: blocks[1].hash(),
                    new_height: 2,
                    num_blocks_cemented: 2
                },
                false
            )
        );
        assert_blocks_equal(&cemented_blocks, &blocks[0..2]);
        assert_eq!(command_factory.is_done(), false);

        // Cement second batch
        let command = command_factory
            .cement(&load_block, &mut cemented_blocks)
            .unwrap()
            .expect("command was None!");

        assert_eq!(
            command,
            (
                ConfirmationHeightUpdate {
                    account: Account::from(42),
                    new_cemented_frontier: blocks[2].hash(),
                    new_height: 3,
                    num_blocks_cemented: 1
                },
                true
            )
        );
        assert_blocks_equal(&cemented_blocks, &blocks[2..]);
        assert!(command_factory.is_done());
    }

    fn create_block_loader(blocks: &[BlockEnum]) -> Box<dyn Fn(&BlockHash) -> Option<BlockEnum>> {
        let map: HashMap<BlockHash, BlockEnum> =
            blocks.iter().map(|b| (b.hash(), b.clone())).collect();
        Box::new(move |block_hash| map.get(block_hash).cloned())
    }

    struct AccountBlocksBuilder {
        account: Account,
        blocks: Vec<BlockEnum>,
    }

    impl AccountBlocksBuilder {
        fn for_account<T: Into<Account>>(account: T) -> Self {
            Self {
                account: account.into(),
                blocks: Vec::new(),
            }
        }

        fn add_block(mut self) -> Self {
            let previous = self.blocks.last().map(|b| b.hash()).unwrap_or_default();
            self.blocks.push(
                BlockBuilder::state()
                    .account(self.account)
                    .previous(previous)
                    .build(),
            );
            self
        }

        fn build(mut self) -> Vec<BlockEnum> {
            let mut height = self.blocks.len() as u64;
            let mut successor = BlockHash::zero();
            for block in self.blocks.iter_mut().rev() {
                block.set_sideband(BlockSideband {
                    height,
                    successor,
                    account: Account::from(42),
                    balance: Amount::raw(42),
                    details: BlockDetails::new(Epoch::Invalid, false, false, false),
                    source_epoch: Epoch::Epoch2,
                    timestamp: 0,
                });

                height -= 1;
                successor = block.hash();
            }

            self.blocks
        }
    }

    fn assert_blocks_equal(cemented_blocks: &[Arc<BlockEnum>], expected: &[BlockEnum]) {
        assert_eq!(
            cemented_blocks
                .iter()
                .map(|b| b.deref().clone())
                .collect::<Vec<_>>(),
            expected
        );
    }
}
