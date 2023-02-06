use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use rsnano_core::{BlockEnum, BlockHash};
use rsnano_ledger::Ledger;
use rsnano_store_traits::Transaction;

use super::{
    block_cache::BlockCache, confirmation_height_unbounded::ReceiveSourcePair,
    implicit_receive_cemented_mapping::ImplictReceiveCementedMapping, ConfHeightDetails,
};

/// Walks backwards through the accounts blocks (starting at current_block)
/// and finds all unconfirmed receives and adds them to receive_source_pairs
pub(crate) struct UnconfirmedReceiveAndSourcesCollector<'a> {
    pub txn: &'a dyn Transaction,
    pub current_block: Arc<BlockEnum>,
    pub confirmation_height: u64,
    pub receive_source_pairs: &'a mut Vec<Arc<ReceiveSourcePair>>,
    pub cemented_by_current_block: &'a mut Vec<BlockHash>,
    pub cemented_by_original_block: &'a mut Vec<BlockHash>,
    pub original_block: &'a BlockEnum,
    pub block_cache: &'a BlockCache,
    pub stopped: &'a AtomicBool,
    pub ledger: &'a Ledger,
    pub implicit_receive_cemented_mapping: &'a mut ImplictReceiveCementedMapping,
    pub first_iter: bool,
    pub hit_receive: bool,
}

impl<'a> UnconfirmedReceiveAndSourcesCollector<'a> {
    pub(crate) fn collect(&mut self) {
        let account = self.current_block.account_calculated();
        let mut block_hash = self.current_block.hash();
        let mut num_to_confirm =
            self.current_block.sideband().unwrap().height - self.confirmation_height;

        // Handle any sends above a receive
        let mut is_original_block = block_hash == self.original_block.hash();
        while (num_to_confirm > 0) && !block_hash.is_zero() && !self.stopped.load(Ordering::SeqCst)
        {
            if self.first_iter {
                self.block_cache.add(Arc::clone(&self.current_block));
            } else {
                match self.block_cache.load_block(&block_hash, self.txn) {
                    Some(block) => {
                        self.current_block = block;
                    }
                    None => {
                        continue;
                    }
                }
            };

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

                let details = ConfHeightDetails {
                    account,
                    latest_confirmed_block: block_hash,
                    new_height: self.confirmation_height + num_to_confirm,
                    num_blocks_confirmed: 1,
                    cemented_in_current_account: vec![block_hash],
                    cemented_in_source: Vec::new(),
                };
                self.receive_source_pairs.push(Arc::new(ReceiveSourcePair {
                    receive_details: Arc::new(Mutex::new(details)),
                    source_hash: self.current_block.source_or_link(),
                }));
            } else if is_original_block {
                self.cemented_by_original_block.push(block_hash);
            } else {
                if !self.hit_receive {
                    // This block is cemented via a receive, as opposed to below a receive being cemented
                    self.cemented_by_current_block.push(block_hash);
                } else {
                    // We have hit a receive before, add the block to it
                    let last_pair = self.receive_source_pairs.last().unwrap();
                    let last_receive_details = &last_pair.receive_details;
                    let mut last_receive_details_lock = last_receive_details.lock().unwrap();
                    last_receive_details_lock.num_blocks_confirmed += 1;
                    last_receive_details_lock
                        .cemented_in_current_account
                        .push(block_hash);
                    drop(last_receive_details_lock);

                    self.implicit_receive_cemented_mapping
                        .add(block_hash, last_receive_details);
                }
            }

            block_hash = self.current_block.previous();

            num_to_confirm -= 1;
            self.first_iter = false;
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
