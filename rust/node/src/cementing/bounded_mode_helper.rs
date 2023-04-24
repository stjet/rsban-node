use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::{Account, BlockHash, ConfirmationHeightInfo, Epochs};

use super::{
    ledger_data_requester::LedgerDataRequester, AccountsConfirmedMap,
    AccountsConfirmedMapContainerInfo, ConfirmedInfo, WriteDetails,
};

/** The maximum number of blocks to be read in while iterating over a long account chain */
const BATCH_READ_SIZE: usize = 65536;

/** The maximum number of various containers to keep the memory bounded */
const MAX_ITEMS: usize = 131072;

#[derive(PartialEq, Eq, Debug)]
pub(crate) enum BoundedCementationStep {
    Write([Option<WriteDetails>; 2]),
    AlreadyCemented(BlockHash),
    Done,
}

#[derive(Clone, Default)]
struct TopAndNextHash {
    pub top: BlockHash,
    pub next: Option<BlockHash>,
    pub next_height: u64,
}

struct ReceiveSourcePair {
    pub receive_details: ReceiveChainDetails,
    pub source_hash: BlockHash,
}

#[derive(Clone)]
struct ReceiveChainDetails {
    pub account: Account,
    pub height: u64,
    pub hash: BlockHash,
    pub top_level: BlockHash,
    pub next: Option<BlockHash>,
    pub bottom_height: u64,
    pub bottom_most: BlockHash,
}

#[derive(Default)]
pub(crate) struct BoundedModeHelperBuilder {
    epochs: Option<Epochs>,
    stopped: Option<Arc<AtomicBool>>,
    receives_per_checkpoint: Option<usize>,
}

impl BoundedModeHelperBuilder {
    pub fn receives_per_checkpoint(mut self, receive_count: usize) -> Self {
        self.receives_per_checkpoint = Some(receive_count);
        self
    }

    pub fn build(self) -> BoundedModeHelper {
        let epochs = self.epochs.unwrap_or_default();
        let stopped = self
            .stopped
            .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));

        let mut helper = BoundedModeHelper::new(epochs, stopped);
        if let Some(receive_count) = self.receives_per_checkpoint {
            helper.receives_per_checkpoint = receive_count;
        }

        helper
    }
}

pub(crate) struct BoundedModeHelper {
    stopped: Arc<AtomicBool>,
    epochs: Epochs,
    next_in_receive_chain: Option<TopAndNextHash>,
    checkpoints: BoundedVecDeque<BlockHash>,
    receive_source_pairs: BoundedVecDeque<ReceiveSourcePair>,
    accounts_confirmed_info: AccountsConfirmedMap,
    current_hash: BlockHash,
    first_iter: bool,
    original_block: BlockHash,
    receive_details: Option<ReceiveChainDetails>,
    hash_to_process: TopAndNextHash,
    top_level_hash: BlockHash,
    account: Account,
    block_height: u64,
    previous: BlockHash,
    current_confirmation_height: ConfirmationHeightInfo,
    top_most_non_receive_block_hash: BlockHash,
    /// Create a checkpoint every time this count of receives were hit
    receives_per_checkpoint: usize,
}

impl BoundedModeHelper {
    pub fn new(epochs: Epochs, stopped: Arc<AtomicBool>) -> Self {
        Self {
            epochs,
            stopped,
            checkpoints: BoundedVecDeque::new(MAX_ITEMS),
            receive_source_pairs: BoundedVecDeque::new(MAX_ITEMS),
            accounts_confirmed_info: AccountsConfirmedMap::new(),
            next_in_receive_chain: None,
            current_hash: BlockHash::zero(),
            first_iter: true,
            original_block: BlockHash::zero(),
            receive_details: None,
            hash_to_process: Default::default(),
            top_level_hash: BlockHash::zero(),
            account: Account::zero(),
            block_height: 0,
            previous: BlockHash::zero(),
            current_confirmation_height: Default::default(),
            top_most_non_receive_block_hash: BlockHash::zero(),
            receives_per_checkpoint: MAX_ITEMS,
        }
    }

    pub fn builder() -> BoundedModeHelperBuilder {
        Default::default()
    }

    pub fn initialize(&mut self, original_block: BlockHash) {
        self.checkpoints.clear();
        self.receive_source_pairs.clear();
        self.next_in_receive_chain = None;
        self.current_hash = BlockHash::zero();
        self.first_iter = true;
        self.original_block = original_block;
        self.receive_details = None;
        self.hash_to_process = Default::default();
        self.top_level_hash = BlockHash::zero();
        self.account = Account::zero();
        self.block_height = 0;
        self.previous = BlockHash::zero();
        self.current_confirmation_height = Default::default();
        self.top_most_non_receive_block_hash = BlockHash::zero();
    }

    pub fn get_next_step<T: LedgerDataRequester>(
        &mut self,
        data_requester: &mut T,
    ) -> BoundedCementationStep {
        loop {
            if !self.first_iter && self.is_done() {
                return BoundedCementationStep::Done;
            }

            while !self.load_next_block(data_requester) {
                if self.current_hash == self.original_block {
                    // The block we are trying to cement was already cemented and pruned!
                    return BoundedCementationStep::Done;
                }
            }

            // This block was added to the confirmation height processor but is already cemented
            if self.original_block_already_cemented() {
                return BoundedCementationStep::AlreadyCemented(self.original_block);
            }

            self.goto_least_unconfirmed_hash(data_requester);
            self.top_most_non_receive_block_hash = self.current_hash;

            let hit_receive = if !self.is_already_cemented() {
                self.iterate(data_requester)
            } else {
                false
            };

            // next_in_receive_chain can be modified when writing, so need to cache it here before resetting
            let is_set = self.next_in_receive_chain.is_some();
            self.next_in_receive_chain = None;
            self.first_iter = false;

            // Need to also handle the case where we are hitting receives where the sends below should be confirmed
            if !hit_receive
                || (self.receive_source_pairs.len() == 1
                    && self.top_most_non_receive_block_hash != self.current_hash)
            {
                let write_next = self.write_next(data_requester, is_set);
                if write_next.iter().any(Option::is_some) {
                    return BoundedCementationStep::Write(write_next);
                }
            }
        }
    }

    pub fn is_accounts_cache_full(&self) -> bool {
        self.accounts_confirmed_info.len() >= MAX_ITEMS
    }

    pub fn is_done(&self) -> bool {
        (self.receive_source_pairs.is_empty() && self.current_hash == self.original_block)
            || self.stopped.load(Ordering::SeqCst)
    }

    fn load_next_block<T: LedgerDataRequester>(&mut self, data_requester: &T) -> bool {
        self.receive_details = None;
        self.hash_to_process = self.get_next_block_hash();
        self.current_hash = self.hash_to_process.top;
        self.top_level_hash = self.current_hash;

        let current_block = data_requester.get_block(&self.current_hash);

        let Some(current_block) = current_block else{
                if data_requester.was_block_pruned(&self.current_hash){
                    if !self.receive_source_pairs.is_empty() {
                        self.receive_source_pairs.pop_back();
                    }
                    return false;
                } else {
                    panic!("Ledger mismatch trying to set confirmation height for block {} (bounded processor)", self.current_hash);
                }
            };

        self.account = current_block.account_calculated();
        self.block_height = current_block.sideband().unwrap().height;
        self.previous = current_block.previous();
        self.current_confirmation_height = self.get_confirmation_height(
            &self.account,
            data_requester,
            &self.accounts_confirmed_info,
        );

        true
    }

    /// The next block hash to iterate over, the priority is as follows:
    /// 1 - The next block in the account chain for the last processed receive (if there is any)
    /// 2 - The next receive block which is closest to genesis
    /// 3 - The last checkpoint hit.
    /// 4 - The hash that was passed in originally. Either all checkpoints were exhausted (this can happen when there are many accounts to genesis)
    ///     or all other blocks have been processed.
    fn get_next_block_hash(&mut self) -> TopAndNextHash {
        if let Some(next_in_chain) = &self.next_in_receive_chain {
            next_in_chain.clone()
        } else if let Some(next_receive_source_pair) = self.receive_source_pairs.back() {
            self.receive_details = Some(next_receive_source_pair.receive_details.clone());
            TopAndNextHash {
                top: next_receive_source_pair.source_hash,
                next: next_receive_source_pair.receive_details.next,
                next_height: next_receive_source_pair.receive_details.height + 1,
            }
        } else if let Some(checkpoint) = self.checkpoints.back() {
            TopAndNextHash {
                top: *checkpoint,
                next: None,
                next_height: 0,
            }
        } else {
            TopAndNextHash {
                top: self.original_block,
                next: None,
                next_height: 0,
            }
        }
    }

    fn get_confirmation_height<T: LedgerDataRequester>(
        &self,
        account: &Account,
        data_requester: &T,
        accounts_confirmed_info: &AccountsConfirmedMap,
    ) -> ConfirmationHeightInfo {
        // Checks if we have encountered this account before but not commited changes yet, if so then update the cached confirmation height
        if let Some(found_info) = accounts_confirmed_info.get(account) {
            ConfirmationHeightInfo::new(found_info.confirmed_height, found_info.iterated_frontier)
        } else {
            data_requester.get_current_confirmation_height(account)
        }
    }

    fn is_already_cemented(&self) -> bool {
        self.current_confirmation_height.height >= self.block_height
    }

    fn original_block_already_cemented(&self) -> bool {
        self.first_iter && self.is_already_cemented() && self.current_hash == self.original_block
    }

    fn blocks_to_cement_for_this_account(&self) -> u64 {
        // If we are not already at the bottom of the account chain (1 above cemented frontier) then find it
        if self.is_already_cemented() {
            0
        } else {
            self.block_height - self.current_confirmation_height.height
        }
    }

    fn get_least_unconfirmed_hash_from_top_level<T: LedgerDataRequester>(
        &mut self,
        data_requester: &T,
    ) -> BlockHash {
        let mut least_unconfirmed_hash = self.current_hash;
        if self.current_confirmation_height.height != 0 {
            if self.block_height > self.current_confirmation_height.height {
                let block = data_requester
                    .get_block(&self.current_confirmation_height.frontier)
                    .unwrap();
                least_unconfirmed_hash = block.sideband().unwrap().successor;
                self.block_height = block.sideband().unwrap().height + 1;
            }
        } else {
            // No blocks have been confirmed, so the first block will be the open block
            let info = data_requester.get_account_info(&self.account).unwrap();
            least_unconfirmed_hash = info.open_block;
            self.block_height = 1;
        }
        return least_unconfirmed_hash;
    }

    fn goto_least_unconfirmed_hash<T: LedgerDataRequester>(&mut self, data_requester: &T) {
        let blocks_to_cement = self.blocks_to_cement_for_this_account();
        if blocks_to_cement < 2 {
            // nothing to do
        } else if blocks_to_cement == 2 {
            // If there is 1 uncemented block in-between this block and the cemented frontier,
            // we can just use the previous block to get the least unconfirmed hash.
            self.current_hash = self.previous;
            self.block_height -= 1;
        } else if self.next_in_receive_chain.is_none() {
            self.current_hash = self.get_least_unconfirmed_hash_from_top_level(data_requester);
        } else {
            // Use the cached successor of the last receive which saves having to do more IO in get_least_unconfirmed_hash_from_top_level
            // as we already know what the next block we should process should be.
            self.current_hash = self.hash_to_process.next.unwrap();
            self.block_height = self.hash_to_process.next_height;
        }
    }

    fn iterate<T: LedgerDataRequester>(&mut self, data_requester: &mut T) -> bool {
        let mut reached_target = false;
        let mut hit_receive = false;
        let mut hash = self.current_hash;
        let mut num_blocks = 0;
        while !hash.is_zero() && !reached_target && !self.stopped.load(Ordering::SeqCst) {
            // Keep iterating upwards until we either reach the desired block or the second receive.
            // Once a receive is cemented, we can cement all blocks above it until the next receive, so store those details for later.
            num_blocks += 1;
            let block = data_requester.get_block(&hash).unwrap();
            let source = block.source_or_link();
            //----------------------------------------
            if !source.is_zero()
                && !self.epochs.is_epoch_link(&source.into())
                && data_requester.get_block(&source).is_some()
            {
                hit_receive = true;
                reached_target = true;
                let sideband = block.sideband().unwrap();
                let next =
                    if !sideband.successor.is_zero() && sideband.successor != self.top_level_hash {
                        Some(sideband.successor)
                    } else {
                        None
                    };
                self.receive_source_pairs.push_back(ReceiveSourcePair {
                    receive_details: ReceiveChainDetails {
                        account: self.account,
                        height: sideband.height,
                        hash,
                        top_level: self.top_level_hash,
                        next,
                        bottom_height: self.block_height,
                        bottom_most: self.current_hash,
                    },
                    source_hash: source,
                });

                // Store a checkpoint every max_items so that we can always traverse a long number of accounts to genesis
                if self.receive_source_pairs.len() % self.receives_per_checkpoint == 0 {
                    self.checkpoints.push_back(self.top_level_hash);
                }
            } else {
                // Found a send/change/epoch block which isn't the desired top level
                self.top_most_non_receive_block_hash = hash;
                if hash == self.top_level_hash {
                    reached_target = true;
                } else {
                    hash = block.sideband().unwrap().successor;
                }
            }

            // We could be traversing a very large account so we don't want to open read transactions for too long.
            if (num_blocks > 0) && num_blocks % BATCH_READ_SIZE == 0 {
                data_requester.refresh_transaction();
            }
        }
        hit_receive
    }

    /// Add the non-receive blocks iterated for this account
    fn cement_non_receive_blocks_for_this_account<T: LedgerDataRequester>(
        &mut self,
        data_requester: &T,
    ) -> Option<WriteDetails> {
        if self.is_already_cemented() {
            return None;
        }

        let height = data_requester
            .get_block(&self.top_most_non_receive_block_hash)
            .map(|b| b.sideband().unwrap().height)
            .unwrap_or_default();

        if height <= self.current_confirmation_height.height {
            return None;
        }

        let confirmed_info = ConfirmedInfo {
            confirmed_height: height,
            iterated_frontier: self.top_most_non_receive_block_hash,
        };

        self.accounts_confirmed_info
            .insert(self.account, confirmed_info);

        truncate_after(&mut self.checkpoints, &self.top_most_non_receive_block_hash);

        Some(WriteDetails {
            account: self.account,
            bottom_height: self.block_height,
            bottom_hash: self.current_hash,
            top_height: height,
            top_hash: self.top_most_non_receive_block_hash,
        })
    }

    /// Add the receive block and all non-receive blocks above that one
    fn cement_receive_block_and_all_non_receive_blocks_above(&mut self) -> Option<WriteDetails> {
        let Some(receive_details) = &self.receive_details else { return None; };
        self.accounts_confirmed_info.insert(
            receive_details.account,
            ConfirmedInfo {
                confirmed_height: receive_details.height,
                iterated_frontier: receive_details.hash,
            },
        );

        if receive_details.next.is_some() {
            self.next_in_receive_chain = Some(TopAndNextHash {
                top: receive_details.top_level,
                next: receive_details.next,
                next_height: receive_details.height + 1,
            });
        } else {
            truncate_after(&mut self.checkpoints, &receive_details.hash);
        }

        Some(WriteDetails {
            account: receive_details.account,
            bottom_height: receive_details.bottom_height,
            bottom_hash: receive_details.bottom_most,
            top_height: receive_details.height,
            top_hash: receive_details.hash,
        })
    }

    pub fn clear_all_cached_accounts(&mut self) {
        self.accounts_confirmed_info.clear();
    }

    pub fn clear_cached_account(&mut self, account: &Account, height: u64) {
        if let Some(found_info) = self.accounts_confirmed_info.get(account) {
            if found_info.confirmed_height == height {
                self.accounts_confirmed_info.remove(account);
            }
        }
    }

    fn write_next<T: LedgerDataRequester>(
        &mut self,
        data_requester: &T,
        is_set: bool,
    ) -> [Option<WriteDetails>; 2] {
        // Once the path to genesis has been iterated to, we can begin to cement the lowest blocks in the accounts. This sets up
        // the non-receive blocks which have been iterated for an account, and the associated receive block.
        let cement_non_receives = self.cement_non_receive_blocks_for_this_account(data_requester);

        let cement_receive = self.cement_receive_block_and_all_non_receive_blocks_above();

        // If used the top level, don't pop off the receive source pair because it wasn't used
        if !is_set && !self.receive_source_pairs.is_empty() {
            self.receive_source_pairs.pop_back();
        }

        [cement_non_receives, cement_receive]
    }

    pub(crate) fn container_info(&self) -> AccountsConfirmedMapContainerInfo {
        self.accounts_confirmed_info.container_info()
    }
}

fn truncate_after(buffer: &mut BoundedVecDeque<BlockHash>, hash: &BlockHash) {
    if let Some((index, _)) = buffer.iter().enumerate().find(|(_, h)| *h != hash) {
        buffer.truncate(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cementing::LedgerDataRequesterStub;
    use rsnano_core::BlockChainBuilder;

    #[test]
    fn cement_first_send_from_genesis() {
        let mut sut = BoundedModeHelper::builder().build();
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block().legacy_send();
        data_requester.add_uncemented(&genesis_chain);

        sut.initialize(genesis_chain.frontier());
        let step = sut.get_next_step(&mut data_requester);

        let expected = WriteDetails {
            account: genesis_chain.account(),
            bottom_height: 2,
            bottom_hash: genesis_chain.frontier(),
            top_height: 2,
            top_hash: genesis_chain.frontier(),
        };
        assert_eq!(step, BoundedCementationStep::Write([Some(expected), None]));
    }

    #[test]
    fn when_done_there_are_no_more_steps() {
        let mut sut = BoundedModeHelper::builder().build();
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block().legacy_send();
        data_requester.add_uncemented(&genesis_chain);

        sut.initialize(genesis_chain.frontier());
        sut.get_next_step(&mut data_requester); // cement the block
        let step = sut.get_next_step(&mut data_requester);
        assert_eq!(step, BoundedCementationStep::Done);
    }

    #[test]
    fn cement_two_blocks_in_one_go() {
        let mut sut = BoundedModeHelper::builder().build();
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester
            .add_genesis_block()
            .legacy_send()
            .legacy_send();
        let first_send = genesis_chain.blocks()[1].hash();
        let second_send = genesis_chain.blocks()[2].hash();
        data_requester.add_uncemented(&genesis_chain);

        sut.initialize(second_send);
        let step = sut.get_next_step(&mut data_requester);

        let expected = WriteDetails {
            account: genesis_chain.account(),
            bottom_height: 2,
            bottom_hash: first_send,
            top_height: 3,
            top_hash: second_send,
        };
        assert_eq!(step, BoundedCementationStep::Write([Some(expected), None]));
    }

    #[test]
    fn cement_three_blocks_in_one_go() {
        let mut sut = BoundedModeHelper::builder().build();
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester
            .add_genesis_block()
            .legacy_send()
            .legacy_send()
            .legacy_send();
        data_requester.add_uncemented(&genesis_chain);

        sut.initialize(genesis_chain.frontier());
        let step = sut.get_next_step(&mut data_requester);

        let expected = WriteDetails {
            account: genesis_chain.account(),
            bottom_height: 2,
            bottom_hash: genesis_chain.blocks()[1].hash(),
            top_height: 4,
            top_hash: genesis_chain.frontier(),
        };
        assert_eq!(step, BoundedCementationStep::Write([Some(expected), None]));
    }

    #[test]
    fn cement_open_block() {
        let mut sut = BoundedModeHelper::builder().build();
        let mut data_requester = LedgerDataRequesterStub::new();
        let dest_chain = BlockChainBuilder::new();
        let genesis_chain = data_requester
            .add_genesis_block()
            .legacy_open()
            .legacy_send_with(|b| b.destination(dest_chain.account()));
        let dest_chain = dest_chain.legacy_open_from(genesis_chain.latest_block());
        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&dest_chain);

        sut.initialize(dest_chain.frontier());
        let step = sut.get_next_step(&mut data_requester);

        let expected = WriteDetails {
            account: dest_chain.account(),
            bottom_height: 1,
            bottom_hash: dest_chain.frontier(),
            top_height: 1,
            top_hash: dest_chain.frontier(),
        };
        assert_eq!(step, BoundedCementationStep::Write([None, Some(expected)]));
        assert_eq!(
            sut.get_next_step(&mut data_requester),
            BoundedCementationStep::Done
        );
        assert!(sut.is_done());
    }

    #[test]
    fn cement_open_block_and_successor_in_one_go() {
        let mut sut = BoundedModeHelper::builder().build();
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester
            .add_genesis_block()
            .legacy_open()
            .legacy_send();
        let dest_chain =
            BlockChainBuilder::from_send_block(genesis_chain.latest_block()).legacy_send();
        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&dest_chain);

        sut.initialize(dest_chain.frontier());
        let step_1 = sut.get_next_step(&mut data_requester);

        let expected = WriteDetails {
            account: dest_chain.account(),
            bottom_height: 1,
            bottom_hash: dest_chain.blocks()[0].hash(),
            top_height: 1,
            top_hash: dest_chain.blocks()[0].hash(),
        };
        assert_eq!(
            step_1,
            BoundedCementationStep::Write([None, Some(expected)])
        );

        let step_2 = sut.get_next_step(&mut data_requester);

        let expected = WriteDetails {
            account: dest_chain.account(),
            bottom_height: 2,
            bottom_hash: dest_chain.frontier(),
            top_height: 2,
            top_hash: dest_chain.frontier(),
        };
        assert_eq!(
            step_2,
            BoundedCementationStep::Write([Some(expected), None])
        );
        assert_eq!(
            sut.get_next_step(&mut data_requester),
            BoundedCementationStep::Done
        );
        assert!(sut.is_done());
    }

    #[test]
    fn cement_open_block_and_two_successors_in_one_go() {
        let mut sut = BoundedModeHelper::builder().build();
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester
            .add_genesis_block()
            .legacy_open()
            .legacy_send();
        let dest_chain = BlockChainBuilder::from_send_block(genesis_chain.latest_block())
            .legacy_send()
            .legacy_send();
        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&dest_chain);

        sut.initialize(dest_chain.frontier());
        let step_1 = sut.get_next_step(&mut data_requester);

        let expected = WriteDetails {
            account: dest_chain.account(),
            bottom_height: 1,
            bottom_hash: dest_chain.blocks()[0].hash(),
            top_height: 1,
            top_hash: dest_chain.blocks()[0].hash(),
        };
        assert_eq!(
            step_1,
            BoundedCementationStep::Write([None, Some(expected)])
        );

        let step_2 = sut.get_next_step(&mut data_requester);

        let expected = WriteDetails {
            account: dest_chain.account(),
            bottom_height: 2,
            bottom_hash: dest_chain.blocks()[1].hash(),
            top_height: 3,
            top_hash: dest_chain.frontier(),
        };
        assert_eq!(
            step_2,
            BoundedCementationStep::Write([Some(expected), None])
        );
        assert_eq!(
            sut.get_next_step(&mut data_requester),
            BoundedCementationStep::Done
        );
        assert!(sut.is_done());
    }

    #[test]
    fn cement_two_accounts_in_one_go() {
        let mut sut = BoundedModeHelper::builder().build();
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester
            .add_genesis_block()
            .legacy_open()
            .legacy_send();
        let dest_1 = BlockChainBuilder::from_send_block(genesis_chain.latest_block())
            .legacy_send()
            .legacy_send()
            .legacy_send_with(|b| b.destination(Account::from(7)));
        let dest_2 = BlockChainBuilder::from_send_block(dest_1.latest_block())
            .legacy_send()
            .legacy_send()
            .legacy_send();

        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&dest_1);
        data_requester.add_uncemented(&dest_2);

        sut.initialize(dest_2.frontier());

        let step_1 = sut.get_next_step(&mut data_requester);
        let expected = WriteDetails {
            account: dest_1.account(),
            bottom_height: 1,
            bottom_hash: dest_1.blocks()[0].hash(),
            top_height: 1,
            top_hash: dest_1.blocks()[0].hash(),
        };
        assert_eq!(
            step_1,
            BoundedCementationStep::Write([None, Some(expected)])
        );

        let step_2 = sut.get_next_step(&mut data_requester);
        let expected = WriteDetails {
            account: dest_1.account(),
            bottom_height: 2,
            bottom_hash: dest_1.blocks()[1].hash(),
            top_height: 4,
            top_hash: dest_1.frontier(),
        };
        assert_eq!(
            step_2,
            BoundedCementationStep::Write([Some(expected), None])
        );

        let step_3 = sut.get_next_step(&mut data_requester);
        let expected = WriteDetails {
            account: dest_2.account(),
            bottom_height: 1,
            bottom_hash: dest_2.blocks()[0].hash(),
            top_height: 1,
            top_hash: dest_2.blocks()[0].hash(),
        };
        assert_eq!(
            step_3,
            BoundedCementationStep::Write([None, Some(expected)])
        );

        let step_4 = sut.get_next_step(&mut data_requester);
        let expected = WriteDetails {
            account: dest_2.account(),
            bottom_height: 2,
            bottom_hash: dest_2.blocks()[1].hash(),
            top_height: 4,
            top_hash: dest_2.frontier(),
        };
        assert_eq!(
            step_4,
            BoundedCementationStep::Write([Some(expected), None])
        );
        assert_eq!(
            sut.get_next_step(&mut data_requester),
            BoundedCementationStep::Done
        );
        assert!(sut.is_done());
    }

    #[test]
    fn block_already_cemented() {
        let mut sut = BoundedModeHelper::builder().build();
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block();

        sut.initialize(genesis_chain.frontier());
        let step = sut.get_next_step(&mut data_requester);

        assert_eq!(
            step,
            BoundedCementationStep::AlreadyCemented(genesis_chain.frontier())
        );
    }

    #[test]
    fn create_checkpoint() {
        let mut sut = BoundedModeHelper::builder()
            .receives_per_checkpoint(2)
            .build();

        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester
            .add_genesis_block()
            .legacy_send_with(|b| b.destination(Account::from(1)));

        let account1 = BlockChainBuilder::from_send_block(genesis_chain.latest_block())
            .legacy_send_with(|b| b.destination(Account::from(2)));

        let account2 = BlockChainBuilder::from_send_block(account1.latest_block())
            .legacy_send_with(|b| b.destination(Account::from(3)));

        let account3 = BlockChainBuilder::from_send_block(account2.latest_block())
            .legacy_send_with(|b| b.destination(Account::from(4)));

        let account4 = BlockChainBuilder::from_send_block(account3.latest_block())
            .legacy_send_with(|b| b.destination(Account::from(5)));

        let account5 = BlockChainBuilder::from_send_block(account4.latest_block())
            .legacy_send_with(|b| b.destination(Account::from(6)));

        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&account1);
        data_requester.add_uncemented(&account2);
        data_requester.add_uncemented(&account3);
        data_requester.add_uncemented(&account4);
        data_requester.add_uncemented(&account5);

        sut.initialize(account5.frontier());

        assert_eq!(
            sut.get_next_step(&mut data_requester),
            BoundedCementationStep::Write([
                None,
                Some(WriteDetails {
                    account: account1.account(),
                    bottom_height: 1,
                    bottom_hash: account1.open(),
                    top_height: 1,
                    top_hash: account1.open(),
                })
            ])
        );
        assert_eq!(
            sut.get_next_step(&mut data_requester),
            BoundedCementationStep::Write([
                Some(WriteDetails {
                    account: account1.account(),
                    bottom_height: 2,
                    bottom_hash: account1.frontier(),
                    top_height: 2,
                    top_hash: account1.frontier(),
                }),
                Some(WriteDetails {
                    account: account2.account(),
                    bottom_height: 1,
                    bottom_hash: account2.open(),
                    top_height: 1,
                    top_hash: account2.open(),
                }),
            ])
        );
        assert_eq!(
            sut.get_next_step(&mut data_requester),
            BoundedCementationStep::Write([
                Some(WriteDetails {
                    account: account2.account(),
                    bottom_height: 2,
                    bottom_hash: account2.frontier(),
                    top_height: 2,
                    top_hash: account2.frontier(),
                }),
                Some(WriteDetails {
                    account: account3.account(),
                    bottom_height: 1,
                    bottom_hash: account3.open(),
                    top_height: 1,
                    top_hash: account3.open(),
                }),
            ])
        );
        assert_eq!(
            sut.get_next_step(&mut data_requester),
            BoundedCementationStep::Write([
                Some(WriteDetails {
                    account: account3.account(),
                    bottom_height: 2,
                    bottom_hash: account3.frontier(),
                    top_height: 2,
                    top_hash: account3.frontier(),
                }),
                Some(WriteDetails {
                    account: account4.account(),
                    bottom_height: 1,
                    bottom_hash: account4.open(),
                    top_height: 1,
                    top_hash: account4.open(),
                }),
            ])
        );
        assert_eq!(
            sut.get_next_step(&mut data_requester),
            BoundedCementationStep::Write([
                Some(WriteDetails {
                    account: account4.account(),
                    bottom_height: 2,
                    bottom_hash: account4.frontier(),
                    top_height: 2,
                    top_hash: account4.frontier(),
                }),
                Some(WriteDetails {
                    account: account5.account(),
                    bottom_height: 1,
                    bottom_hash: account5.open(),
                    top_height: 1,
                    top_hash: account5.open(),
                }),
            ])
        );
        assert_eq!(
            sut.get_next_step(&mut data_requester),
            BoundedCementationStep::Write([
                Some(WriteDetails {
                    account: account5.account(),
                    bottom_height: 2,
                    bottom_hash: account5.frontier(),
                    top_height: 2,
                    top_hash: account5.frontier(),
                }),
                None
            ])
        );
        assert_eq!(
            sut.get_next_step(&mut data_requester),
            BoundedCementationStep::Done
        );
    }

    mod pruning {
        use super::*;

        #[test]
        fn cement_already_pruned_block() {
            let mut sut = BoundedModeHelper::builder().build();
            let mut data_requester = LedgerDataRequesterStub::new();
            let hash = BlockHash::from(1);
            data_requester.prune(hash);

            sut.initialize(hash);
            let step = sut.get_next_step(&mut data_requester);

            assert_eq!(step, BoundedCementationStep::Done);
        }

        #[test]
        fn send_block_pruned() {
            let mut sut = BoundedModeHelper::builder().build();
            let mut data_requester = LedgerDataRequesterStub::new();

            let genesis_chain = data_requester.add_genesis_block().legacy_send();
            let dest_chain = BlockChainBuilder::from_send_block(genesis_chain.latest_block());
            data_requester.add_cemented(&genesis_chain);
            data_requester.add_uncemented(&dest_chain);
            data_requester.prune(genesis_chain.frontier());

            sut.initialize(dest_chain.frontier());
            let step = sut.get_next_step(&mut data_requester);

            let expected = WriteDetails {
                account: dest_chain.account(),
                bottom_height: 1,
                bottom_hash: dest_chain.frontier(),
                top_height: 1,
                top_hash: dest_chain.frontier(),
            };

            assert_eq!(step, BoundedCementationStep::Write([Some(expected), None]));
        }
    }
}
