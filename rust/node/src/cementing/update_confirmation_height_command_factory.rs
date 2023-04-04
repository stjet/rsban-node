use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use anyhow::Context;
use rsnano_core::{BlockEnum, BlockHash, ConfirmationHeightInfo, UpdateConfirmationHeight};

use super::write_details_queue::WriteDetails;

/// Creates UpdateConfirmationHeight commands for a single account
pub(crate) struct UpdateConfirmationHeightCommandFactory<'a> {
    pending: &'a WriteDetails,
    confirmation_height_info: &'a ConfirmationHeightInfo,
    batch_write_size: &'a AtomicUsize,
    total_blocks_cemented: u64,
    new_cemented_frontier_hash: BlockHash,
    num_blocks_to_cement: u64,
    start_height: u64,
    num_blocks_iterated: u64,
    new_cemented_frontier_block: Option<Arc<BlockEnum>>,
    is_initialized: bool,
}

impl<'a> UpdateConfirmationHeightCommandFactory<'a> {
    pub fn new(
        pending: &'a WriteDetails,
        confirmation_height_info: &'a ConfirmationHeightInfo,
        batch_write_size: &'a AtomicUsize,
    ) -> Self {
        Self {
            pending,
            confirmation_height_info,
            batch_write_size,
            total_blocks_cemented: 0,
            new_cemented_frontier_hash: Default::default(),
            num_blocks_to_cement: 0,
            start_height: 0,
            num_blocks_iterated: 0,
            new_cemented_frontier_block: None,
            is_initialized: false,
        }
    }

    pub fn create_command(
        &mut self,
        load_block: &dyn Fn(BlockHash) -> Option<BlockEnum>,
        cemented_blocks: &mut Vec<Arc<BlockEnum>>,
    ) -> anyhow::Result<Option<UpdateConfirmationHeight>> {
        cemented_blocks.clear();

        if self.pending.top_height <= self.confirmation_height_info.height {
            // No blocks need to be cemented
            return Ok(None);
        }

        if !self.is_initialized {
            self.initialize(load_block)?;
            self.is_initialized = true;
        }

        // Cementing starts from the bottom of the chain and works upwards. This is because chains can have effectively
        // an infinite number of send/change blocks in a row. We don't want to hold the write transaction open for too long.
        for i in self.num_blocks_iterated..self.num_blocks_to_cement {
            self.num_blocks_iterated = i + 1;
            let Some(new_frontier) = &self.new_cemented_frontier_block else { break; };
            cemented_blocks.push(new_frontier.clone());
            self.total_blocks_cemented += 1;

            // Flush these callbacks and continue as we write in batches (ideally maximum 250ms) to not hold write db transaction for too long.
            let update_command = if self.should_flush(&cemented_blocks) {
                Some(self.get_update_command(&cemented_blocks))
            } else {
                None
            };

            self.load_next_block_to_cement(&load_block)
                .with_context(|| {
                    format!(
                        "Could not load next block to cement for account {}",
                        self.pending.account
                    )
                })?;

            if update_command.is_some() {
                return Ok(update_command);
            }
        }

        if cemented_blocks.len() > 0 {
            Ok(Some(UpdateConfirmationHeight {
                account: self.pending.account,
                new_cemented_frontier: self.pending.top_hash,
                new_height: self.pending.top_height,
                num_blocks_cemented: cemented_blocks.len() as u64,
            }))
        } else {
            Ok(None)
        }
    }

    fn initialize(
        &mut self,
        load_block: &dyn Fn(BlockHash) -> Option<BlockEnum>,
    ) -> Result<(), anyhow::Error> {
        if self.pending.bottom_height > self.confirmation_height_info.height {
            // This is the usual case where pending.bottom_height is the first uncemented block
            if self.pending.bottom_height != self.confirmation_height_info.height + 1 {
                bail!(
                    "pending.bottom_height should be exactly 1 block above the cemented frontier!"
                );
            }
            self.new_cemented_frontier_hash = self.pending.bottom_hash;
            self.num_blocks_to_cement = self.pending.top_height - self.pending.bottom_height + 1;
            self.start_height = self.pending.bottom_height;
        } else {
            // Some blocks got cemented already. We have to adjust our starting point
            let current_frontier = self.load_current_cemented_frontier(load_block)?;
            self.new_cemented_frontier_hash = current_frontier.sideband().unwrap().successor;
            self.num_blocks_to_cement =
                self.pending.top_height - self.confirmation_height_info.height;
            self.start_height = self.confirmation_height_info.height + 1;
        }

        self.new_cemented_frontier_block = Some(Arc::new(
            load_block(self.new_cemented_frontier_hash)
                .ok_or_else(|| anyhow!("block not found"))?,
        ));

        Ok(())
    }

    fn load_current_cemented_frontier(
        &mut self,
        load_block: &dyn Fn(BlockHash) -> Option<BlockEnum>,
    ) -> anyhow::Result<BlockEnum> {
        load_block(self.confirmation_height_info.frontier).ok_or_else(|| {
            anyhow!(
                "Could not load current cemented frontier {} for account {}",
                self.confirmation_height_info.frontier,
                self.pending.account
            )
        })
    }

    fn load_next_block_to_cement(
        &mut self,
        load_block: &dyn Fn(BlockHash) -> Option<BlockEnum>,
    ) -> anyhow::Result<()> {
        let Some(current) = &self.new_cemented_frontier_block else { bail!("no current block loaded!") };

        // Get the next block in the chain until we have reached the final desired one
        if !self.is_done() {
            self.new_cemented_frontier_hash = current.sideband().unwrap().successor;
            let next_block = load_block(self.new_cemented_frontier_hash);
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

    fn should_flush(&self, cemented_blocks: &[Arc<BlockEnum>]) -> bool {
        cemented_blocks.len() > self.min_block_count_for_flush()
    }

    fn min_block_count_for_flush(&self) -> usize {
        // Include a tolerance to save having to potentially wait on the block processor if the number of blocks to cement is only a bit higher than the max.
        let size = self.batch_write_size.load(Ordering::SeqCst);
        size + (size / 10)
    }

    fn get_update_command(&self, cemented_blocks: &[Arc<BlockEnum>]) -> UpdateConfirmationHeight {
        UpdateConfirmationHeight {
            account: self.pending.account,
            new_cemented_frontier: self.new_cemented_frontier_hash,
            new_height: self.start_height + self.total_blocks_cemented - 1,
            num_blocks_cemented: cemented_blocks.len() as u64,
        }
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

        let batch_write_size = AtomicUsize::new(42);

        let mut command_factory =
            UpdateConfirmationHeightCommandFactory::new(&pending, &conf_height, &batch_write_size);

        let mut cemented_blocks = Vec::new();
        let command = command_factory
            .create_command(&|_| unimplemented!(), &mut cemented_blocks)
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

        let batch_write_size = AtomicUsize::new(42);

        let mut command_factory =
            UpdateConfirmationHeightCommandFactory::new(&pending, &conf_height, &batch_write_size);

        let mut cemented_blocks = Vec::new();
        let command = command_factory
            .create_command(&|_| unimplemented!(), &mut cemented_blocks)
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

        let batch_write_size = AtomicUsize::new(42);

        let mut command_factory =
            UpdateConfirmationHeightCommandFactory::new(&pending, &conf_height, &batch_write_size);

        let load_block = create_block_loader(&blocks);
        let mut cemented_blocks = Vec::new();
        let command = command_factory
            .create_command(&load_block, &mut cemented_blocks)
            .unwrap()
            .expect("command was None!");

        assert_eq!(
            command,
            UpdateConfirmationHeight {
                account: Account::from(1),
                new_cemented_frontier: blocks[0].hash(),
                new_height: 1,
                num_blocks_cemented: 1
            }
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

        let batch_write_size = AtomicUsize::new(42);

        let mut command_factory =
            UpdateConfirmationHeightCommandFactory::new(&pending, &conf_height, &batch_write_size);

        let load_block = create_block_loader(&blocks);
        let mut cemented_blocks = Vec::new();
        let command = command_factory
            .create_command(&load_block, &mut cemented_blocks)
            .unwrap()
            .expect("command was None!");

        assert_eq!(
            command,
            UpdateConfirmationHeight {
                account: Account::from(42),
                new_cemented_frontier: blocks[1].hash(),
                new_height: 2,
                num_blocks_cemented: 2
            }
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

        let batch_write_size = AtomicUsize::new(42);

        let mut command_factory =
            UpdateConfirmationHeightCommandFactory::new(&pending, &conf_height, &batch_write_size);

        let load_block = create_block_loader(&blocks);
        let mut cemented_blocks = Vec::new();
        let command = command_factory
            .create_command(&load_block, &mut cemented_blocks)
            .unwrap()
            .expect("command was None!");

        assert_eq!(
            command,
            UpdateConfirmationHeight {
                account: Account::from(42),
                new_cemented_frontier: blocks[1].hash(),
                new_height: 2,
                num_blocks_cemented: 1
            }
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
        let batch_write_size = AtomicUsize::new(0);

        let mut command_factory =
            UpdateConfirmationHeightCommandFactory::new(&pending, &conf_height, &batch_write_size);

        let load_block = create_block_loader(&blocks);
        let mut cemented_blocks = Vec::new();

        // Cement first batch
        let command = command_factory
            .create_command(&load_block, &mut cemented_blocks)
            .unwrap()
            .expect("command was None!");

        assert_eq!(
            command,
            UpdateConfirmationHeight {
                account: Account::from(42),
                new_cemented_frontier: blocks[0].hash(),
                new_height: 1,
                num_blocks_cemented: 1
            }
        );
        assert_blocks_equal(&cemented_blocks, &blocks[0..1]);
        assert_eq!(command_factory.is_done(), false);

        // Cement second batch
        let command = command_factory
            .create_command(&load_block, &mut cemented_blocks)
            .unwrap()
            .unwrap();

        assert_eq!(
            command,
            UpdateConfirmationHeight {
                account: Account::from(42),
                new_cemented_frontier: blocks[1].hash(),
                new_height: 2,
                num_blocks_cemented: 1
            }
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
        let batch_write_size = AtomicUsize::new(1);

        let mut command_factory =
            UpdateConfirmationHeightCommandFactory::new(&pending, &conf_height, &batch_write_size);

        let load_block = create_block_loader(&blocks);
        let mut cemented_blocks = Vec::new();

        // Cement first batch
        let command = command_factory
            .create_command(&load_block, &mut cemented_blocks)
            .unwrap()
            .expect("command was None!");

        assert_eq!(
            command,
            UpdateConfirmationHeight {
                account: Account::from(42),
                new_cemented_frontier: blocks[1].hash(),
                new_height: 2,
                num_blocks_cemented: 2
            }
        );
        assert_blocks_equal(&cemented_blocks, &blocks[0..2]);
        assert_eq!(command_factory.is_done(), false);

        // Cement second batch
        let command = command_factory
            .create_command(&load_block, &mut cemented_blocks)
            .unwrap()
            .expect("command was None!");

        assert_eq!(
            command,
            UpdateConfirmationHeight {
                account: Account::from(42),
                new_cemented_frontier: blocks[2].hash(),
                new_height: 3,
                num_blocks_cemented: 1
            }
        );
        assert_blocks_equal(&cemented_blocks, &blocks[2..]);
        assert!(command_factory.is_done());
    }

    fn create_block_loader(blocks: &[BlockEnum]) -> Box<dyn Fn(BlockHash) -> Option<BlockEnum>> {
        let map: HashMap<BlockHash, BlockEnum> =
            blocks.iter().map(|b| (b.hash(), b.clone())).collect();
        Box::new(move |block_hash| map.get(&block_hash).cloned())
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
