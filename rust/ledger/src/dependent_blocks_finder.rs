use crate::ledger::Ledger;
use rsnano_core::{Block, BlockEnum, BlockHash, OpenBlock, StateBlock};
use rsnano_store_lmdb::{Environment, Transaction};

/// Finds all dependent blocks for a given block.
/// There can be at most two dependencies per block, namely "previous" and "link/source".
pub(crate) struct DependentBlocksFinder<'a, T: Environment + 'static> {
    ledger: &'a Ledger<T>,
    txn: &'a dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
}

impl<'a, T: Environment + 'static> DependentBlocksFinder<'a, T> {
    pub(crate) fn new(
        ledger: &'a Ledger<T>,
        txn: &'a dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> Self {
        Self { ledger, txn }
    }

    pub(crate) fn find_dependent_blocks(&self, block: &BlockEnum) -> (BlockHash, BlockHash) {
        match block {
            BlockEnum::LegacySend(_) | BlockEnum::LegacyChange(_) => {
                (block.previous(), BlockHash::zero())
            }
            BlockEnum::LegacyReceive(receive) => (receive.previous(), receive.source()),
            BlockEnum::LegacyOpen(open) => {
                if self.is_genesis_open(open) {
                    // genesis open block does not have any further dependencies
                    Default::default()
                } else {
                    (open.source(), BlockHash::zero())
                }
            }

            BlockEnum::State(state) => {
                let linked_block = if self.link_refers_to_block(state) {
                    state.link().into()
                } else {
                    BlockHash::zero()
                };
                (block.previous(), linked_block)
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
                    .balance(self.txn, &block.previous())
                    .unwrap_or_default()
        }
    }
}
