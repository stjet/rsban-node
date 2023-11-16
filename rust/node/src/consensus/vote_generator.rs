use std::sync::Arc;

use rsnano_core::{BlockHash, Root};
use rsnano_ledger::Ledger;
use rsnano_store_lmdb::LmdbWriteTransaction;

use crate::{
    stats::{StatType, Stats},
    utils::ProcessingQueue,
};

pub struct VoteGenerator {
    ledger: Arc<Ledger>,
    is_final: bool,
    vote_generation_queue: ProcessingQueue<(Root, BlockHash)>,
}

impl VoteGenerator {
    pub fn new(ledger: Arc<Ledger>, is_final: bool, stats: Arc<Stats>) -> Self {
        Self {
            ledger,
            is_final,
            vote_generation_queue: ProcessingQueue::new(
                stats,
                StatType::VoteGenerator,
                "Voting que".to_string(),
                1,         // single threaded
                1024 * 32, // max queue size
                1024 * 4,  // max batch size,
                Box::new(|_batch| {
                    //TODO implement this
                    todo!()
                }),
            ),
        }
    }

    pub fn should_vote(
        &self,
        txn: &mut LmdbWriteTransaction,
        root: &Root,
        hash: &BlockHash,
    ) -> bool {
        if self.is_final {
            match self.ledger.get_block(txn, hash) {
                Some(block) => {
                    debug_assert!(block.root() == *root);
                    self.ledger.dependents_confirmed(txn, &block)
                        && self
                            .ledger
                            .store
                            .final_vote
                            .put(txn, &block.qualified_root(), hash)
                }
                None => false,
            }
        } else {
            match self.ledger.get_block(txn, hash) {
                Some(block) => self.ledger.dependents_confirmed(txn, &block),
                None => false,
            }
        }
    }
}
