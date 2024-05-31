use crate::ledger::Ledger;
use rsnano_core::{Block, BlockEnum, BlockHash, OpenBlock, StateBlock};
use rsnano_store_lmdb::Transaction;

#[derive(Default)]
pub struct DependentBlocks {
    dependents: [BlockHash; 2],
}

impl DependentBlocks {
    pub fn new(previous: BlockHash, link: BlockHash) -> Self {
        Self {
            dependents: [previous, link],
        }
    }

    pub fn previous(&self) -> Option<BlockHash> {
        self.get_index(0)
    }

    pub fn link(&self) -> Option<BlockHash> {
        self.get_index(1)
    }

    fn get_index(&self, index: usize) -> Option<BlockHash> {
        if self.dependents[index].is_zero() {
            None
        } else {
            Some(self.dependents[index])
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &BlockHash> {
        self.dependents
            .iter()
            .flat_map(|i| if i.is_zero() { None } else { Some(i) })
    }
}

/// Finds all dependent blocks for a given block.
/// There can be at most two dependencies per block, namely "previous" and "link/source".
pub struct DependentBlocksFinder<'a> {
    ledger: &'a Ledger,
    txn: &'a dyn Transaction,
}

impl<'a> DependentBlocksFinder<'a> {
    pub fn new(ledger: &'a Ledger, txn: &'a dyn Transaction) -> Self {
        Self { ledger, txn }
    }

    pub fn find_dependent_blocks(&self, block: &BlockEnum) -> DependentBlocks {
        match block {
            BlockEnum::LegacySend(_) | BlockEnum::LegacyChange(_) => {
                DependentBlocks::new(block.previous(), BlockHash::zero())
            }
            BlockEnum::LegacyReceive(receive) => {
                DependentBlocks::new(receive.previous(), receive.source())
            }
            BlockEnum::LegacyOpen(open) => {
                if self.is_genesis_open(open) {
                    // genesis open block does not have any further dependencies
                    Default::default()
                } else {
                    DependentBlocks::new(open.source(), BlockHash::zero())
                }
            }

            BlockEnum::State(state) => {
                let linked_block = if self.link_refers_to_block(state) {
                    state.link().into()
                } else {
                    BlockHash::zero()
                };
                DependentBlocks::new(block.previous(), linked_block)
            }
        }
    }

    fn link_refers_to_block(&self, state: &StateBlock) -> bool {
        !self.ledger.is_epoch_link(&state.link()) && !self.is_send(state)
    }

    fn is_genesis_open(&self, open: &OpenBlock) -> bool {
        open.account() == self.ledger.constants.genesis_account
    }

    // This function is used in place of block.is_send() as it is tolerant to the block not having the sideband information loaded
    // This is needed for instance in vote generation on forks which have not yet had sideband information attached
    fn is_send(&self, block: &StateBlock) -> bool {
        if block.previous().is_zero() {
            return false;
        }
        if let Some(sideband) = block.sideband() {
            sideband.details.is_send
        } else {
            block.balance()
                < self
                    .ledger
                    .any()
                    .block_balance(self.txn, &block.previous())
                    .unwrap_or_default()
        }
    }
}
