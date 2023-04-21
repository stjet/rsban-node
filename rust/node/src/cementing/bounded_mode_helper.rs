use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::{Account, BlockHash, ConfirmationHeightInfo, Epochs};

use super::{
    cementation_data_requester::CementationDataRequester, AccountsConfirmedMap,
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
}

impl BoundedModeHelperBuilder {
    pub fn build(self) -> BoundedModeHelper {
        let epochs = self.epochs.unwrap_or_default();
        let stopped = self
            .stopped
            .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));

        BoundedModeHelper::new(epochs, stopped)
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

    pub fn get_next_step<T: CementationDataRequester>(
        &mut self,
        data_requester: &mut T,
    ) -> BoundedCementationStep {
        loop {
            if self.is_done() {
                return BoundedCementationStep::Done;
            }

            while !self.load_next_block(data_requester) {
                if self.is_done() || self.stopped.load(Ordering::SeqCst) {
                    return BoundedCementationStep::Done;
                }
            }

            // This block was added to the confirmation height processor but is already confirmed
            if self.should_notify_already_cemented() {
                return BoundedCementationStep::AlreadyCemented(self.original_block);
            }

            self.goto_least_unconfirmed_hash(data_requester);
            self.top_most_non_receive_block_hash = self.current_hash;

            let hit_receive = if !self.is_already_cemented() {
                self.iterate(data_requester)
            } else {
                false
            };

            // Exit early when the processor has been stopped, otherwise this function may take a
            // while (and hence keep the process running) if updating a long chain.
            if self.stopped.load(Ordering::SeqCst) {
                return BoundedCementationStep::Done;
            }

            // next_in_receive_chain can be modified when writing, so need to cache it here before resetting
            let is_set = self.next_in_receive_chain.is_some();
            self.next_in_receive_chain = None;
            self.first_iter = false;

            // Need to also handle the case where we are hitting receives where the sends below should be confirmed
            if !hit_receive
                || (self.receive_source_pairs.len() == 1
                    && self.top_most_non_receive_block_hash != self.current_hash)
            {
                return BoundedCementationStep::Write(self.write_next(data_requester, is_set));
            }
        }
    }

    pub fn is_accounts_cache_full(&self) -> bool {
        self.accounts_confirmed_info.len() >= MAX_ITEMS
    }

    pub fn is_done(&self) -> bool {
        !self.first_iter
            && self.receive_source_pairs.is_empty()
            && self.current_hash == self.original_block
    }

    fn load_next_block<T: CementationDataRequester>(&mut self, data_requester: &T) -> bool {
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

    fn get_confirmation_height<T: CementationDataRequester>(
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

    fn should_notify_already_cemented(&self) -> bool {
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

    fn get_least_unconfirmed_hash_from_top_level<T: CementationDataRequester>(
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

    fn goto_least_unconfirmed_hash<T: CementationDataRequester>(&mut self, data_requester: &T) {
        if self.blocks_to_cement_for_this_account() > 1 {
            if self.blocks_to_cement_for_this_account() == 2 {
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
    }

    fn iterate<T: CementationDataRequester>(&mut self, data_requester: &mut T) -> bool {
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
                if self.receive_source_pairs.len() % MAX_ITEMS == 0 {
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
    fn cement_non_receive_blocks_for_this_account<T: CementationDataRequester>(
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

    pub fn clear_accounts_cache(&mut self) {
        self.accounts_confirmed_info.clear();
    }

    pub fn clear_cache(&mut self, account: &Account, height: u64) {
        if let Some(found_info) = self.accounts_confirmed_info.get(account) {
            if found_info.confirmed_height == height {
                self.accounts_confirmed_info.remove(account);
            }
        }
    }

    fn write_next<T: CementationDataRequester>(
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
    use crate::cementing::{
        cementation_data_requester::BlockChainBuilder, CementationDataRequesterStub,
    };

    #[test]
    fn block_already_cemented() {
        let mut sut = BoundedModeHelper::builder().build();
        let mut data_requester = CementationDataRequesterStub::new();
        let mut genesis_chain = BlockChainBuilder::new(Account::from(1));
        let genesis_hash = genesis_chain.legacy_open().hash();
        data_requester.add_cemented(&mut genesis_chain);

        sut.initialize(genesis_hash);
        let step = sut.get_next_step(&mut data_requester);

        assert_eq!(step, BoundedCementationStep::AlreadyCemented(genesis_hash));
    }

    #[test]
    fn cement_first_send_from_genesis() {
        let mut sut = BoundedModeHelper::builder().build();
        let mut data_requester = CementationDataRequesterStub::new();
        let mut genesis_chain = BlockChainBuilder::new(Account::from(1));
        genesis_chain.legacy_open().hash();
        data_requester.add_cemented(&mut genesis_chain);
        let first_send = genesis_chain.legacy_send().hash();
        data_requester.add_uncemented(&mut genesis_chain);

        sut.initialize(first_send);
        let step = sut.get_next_step(&mut data_requester);

        let expected = WriteDetails {
            account: genesis_chain.account,
            bottom_height: 2,
            bottom_hash: first_send,
            top_height: 2,
            top_hash: first_send,
        };
        assert_eq!(step, BoundedCementationStep::Write([Some(expected), None]));
    }

    #[test]
    fn cement_multiple_sends_in_one_go() {
        let mut sut = BoundedModeHelper::builder().build();
        let mut data_requester = CementationDataRequesterStub::new();

        let genesis_account = Account::from(1);
        let mut genesis_chain = BlockChainBuilder::new(genesis_account);
        genesis_chain.legacy_open();
        data_requester.add_cemented(&mut genesis_chain);

        let first_send = genesis_chain.legacy_send().hash();
        let second_send = genesis_chain.legacy_send().hash();
        data_requester.add_uncemented(&mut genesis_chain);

        sut.initialize(second_send);
        let step = sut.get_next_step(&mut data_requester);

        let expected = WriteDetails {
            account: genesis_account,
            bottom_height: 2,
            bottom_hash: first_send,
            top_height: 3,
            top_hash: second_send,
        };
        assert_eq!(step, BoundedCementationStep::Write([Some(expected), None]));
    }

    #[test]
    fn cement_open_block() {
        let mut sut = BoundedModeHelper::builder().build();
        let mut data_requester = CementationDataRequesterStub::new();
        let mut genesis_chain = BlockChainBuilder::new(Account::from(1));
        let mut dest_chain = BlockChainBuilder::new(Account::from(2));

        genesis_chain.legacy_open();
        let send = genesis_chain.legacy_send_with(|b| b.destination(dest_chain.account));
        let open = dest_chain.legacy_open_from(send).hash();
        data_requester.add_cemented(&mut genesis_chain);
        data_requester.add_uncemented(&mut dest_chain);

        sut.initialize(open);
        let step = sut.get_next_step(&mut data_requester);

        let expected = WriteDetails {
            account: dest_chain.account,
            bottom_height: 1,
            bottom_hash: open,
            top_height: 1,
            top_hash: open,
        };
        assert_eq!(step, BoundedCementationStep::Write([None, Some(expected)]));
    }
}
