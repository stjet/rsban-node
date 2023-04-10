use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use rsnano_core::{BlockEnum, BlockHash, ConfirmationHeightUpdate};
use rsnano_ledger::Ledger;
use rsnano_store_traits::Transaction;

use super::{
    block_cache::BlockCache, implicit_receive_cemented_mapping::ImplictReceiveCementedMapping,
    unbounded_mode::ReceiveSourcePair, ConfHeightDetails,
};

/// Walks backwards through the accounts blocks (starting at current_block)
/// and finds all unconfirmed receives and adds them to receive_source_pairs
pub(crate) struct UnconfirmedReceiveAndSourcesCollector<'a> {
    txn: &'a dyn Transaction,
    current_block: Arc<BlockEnum>,
    confirmation_height: u64,
    receive_source_pairs: &'a mut Vec<Arc<ReceiveSourcePair>>,
    cemented_by_current_block: &'a mut Vec<BlockHash>,
    cemented_by_original_block: &'a mut Vec<BlockHash>,
    original_block: &'a BlockEnum,
    block_cache: &'a BlockCache,
    ledger: &'a Ledger,
    implicit_receive_cemented_mapping: &'a mut ImplictReceiveCementedMapping,
    hit_receive: bool,
    num_to_confirm: u64,
}

impl<'a> UnconfirmedReceiveAndSourcesCollector<'a> {
    pub(crate) fn new(
        txn: &'a dyn Transaction,
        current_block: Arc<BlockEnum>,
        confirmation_height: u64,
        receive_source_pairs: &'a mut Vec<Arc<ReceiveSourcePair>>,
        cemented_by_current_block: &'a mut Vec<BlockHash>,
        cemented_by_original_block: &'a mut Vec<BlockHash>,
        original_block: &'a BlockEnum,
        block_cache: &'a BlockCache,
        ledger: &'a Ledger,
        implicit_receive_cemented_mapping: &'a mut ImplictReceiveCementedMapping,
    ) -> Self {
        let num_to_confirm = current_block.sideband().unwrap().height - confirmation_height;
        Self {
            txn,
            current_block,
            confirmation_height,
            receive_source_pairs,
            cemented_by_current_block,
            cemented_by_original_block,
            original_block,
            block_cache,
            ledger,
            implicit_receive_cemented_mapping,
            hit_receive: false,
            num_to_confirm,
        }
    }

    pub(crate) fn collect(&mut self, stopped: &AtomicBool) {
        self.block_cache.add(Arc::clone(&self.current_block));
        let mut is_original_block = self.current_block.hash() == self.original_block.hash();

        while self.num_to_confirm > 0 && !stopped.load(Ordering::SeqCst) {
            if self.is_receive_block() {
                if !self.hit_receive && !self.cemented_by_current_block.is_empty() {
                    // Add the callbacks to the associated receive to retrieve later
                    let last_pair = self.receive_source_pairs.last().unwrap();
                    last_pair.receive_details.lock().unwrap().cemented_in_source =
                        self.cemented_by_current_block.clone();
                    self.cemented_by_current_block.clear();
                }

                is_original_block = false;
                self.hit_receive = true;

                self.add_receive_source_pair();
            } else if is_original_block {
                self.cemented_by_original_block
                    .push(self.current_block.hash());
            } else {
                if !self.hit_receive {
                    // This block is cemented via a receive, as opposed to below a receive being cemented
                    self.cemented_by_current_block
                        .push(self.current_block.hash());
                } else {
                    // We have hit a receive before, add the block to it
                    let last_pair = self.receive_source_pairs.last().unwrap();
                    let last_receive_details = &last_pair.receive_details;
                    let mut last_receive_details_lock = last_receive_details.lock().unwrap();
                    last_receive_details_lock.update_height.num_blocks_cemented += 1;
                    last_receive_details_lock
                        .cemented_in_current_account
                        .push(self.current_block.hash());
                    drop(last_receive_details_lock);

                    self.implicit_receive_cemented_mapping
                        .add(self.current_block.hash(), last_receive_details);
                }
            }

            self.load_previous_block();
        }
    }

    fn add_receive_source_pair(&mut self) {
        let details = self.create_conf_height_details();
        self.receive_source_pairs.push(Arc::new(ReceiveSourcePair {
            receive_details: Arc::new(Mutex::new(details)),
            source_hash: self.current_block.source_or_link(),
        }));
    }

    fn create_conf_height_details(&mut self) -> ConfHeightDetails {
        ConfHeightDetails {
            update_height: ConfirmationHeightUpdate {
                account: self.current_block.account_calculated(),
                new_cemented_frontier: self.current_block.hash(),
                new_height: self.confirmation_height + self.num_to_confirm,
                num_blocks_cemented: 1,
            },
            cemented_in_current_account: vec![self.current_block.hash()],
            cemented_in_source: Vec::new(),
        }
    }

    fn load_previous_block(&mut self) {
        let previous = self.current_block.previous();
        match self.block_cache.load_block(&previous, self.txn) {
            Some(block) => {
                self.current_block = block;
                self.num_to_confirm -= 1;
            }
            None => self.num_to_confirm = 0,
        }
    }

    fn is_receive_block(&self) -> bool {
        let source = self.current_block.source_or_link();

        // a receive block must have a source
        !source.is_zero()
            && !self.ledger.is_epoch_link(&source.into())
            // if source does not point to an existing block then it
            // must be a send block
            && self.ledger.store.block().exists(self.txn, &source)
    }
}
