use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::{Account, BlockEnum, BlockHash, ConfirmationHeightInfo, Epochs};

use super::{
    AccountsConfirmedMap, AccountsConfirmedMapContainerInfo, BlockCache, BlockChainSection,
    ConfirmedInfo, LedgerDataRequester,
};

/** The maximum number of various containers to keep the memory bounded */
const MAX_ITEMS: usize = 131072;

struct BlockRange {
    // inclusive lowest block
    bottom: BlockHash,
    // inclusive highest block
    top: BlockHash,
}

impl BlockRange {
    fn new(bottom: BlockHash, top: BlockHash) -> Self {
        Self { bottom, top }
    }
}

// Data for iterating a single block chain
#[derive(Debug)]
struct ChainIteration {
    account: Account,
    bottom_hash: BlockHash,
    bottom_height: u64,
    top_hash: BlockHash,
    top_height: u64,
    current_hash: BlockHash,
    current_height: u64,
    /// The block after the highest block which we cement. This will become the new lowest uncemented block.
    top_successor: Option<BlockHash>,
}

impl ChainIteration {
    fn new(lowest_block: &BlockEnum, highest_block: &BlockEnum) -> Self {
        debug_assert!(lowest_block.height() <= highest_block.height());
        Self {
            account: highest_block.account_calculated(),
            bottom_hash: lowest_block.hash(),
            bottom_height: lowest_block.height(),
            top_hash: highest_block.hash(),
            top_height: highest_block.height(),
            current_hash: lowest_block.hash(),
            current_height: lowest_block.height(),
            top_successor: highest_block.successor(),
        }
    }

    fn set_done(&mut self) {
        self.current_hash = BlockHash::zero();
        self.current_height = self.top_height + 1;
    }

    fn is_done(&self) -> bool {
        self.current_height > self.top_height
    }

    /// Search range for receive blocks
    fn search_range(&self) -> BlockRange {
        BlockRange::new(self.current_hash, self.top_hash)
    }

    fn go_to_successor_of(&mut self, block: &BlockEnum) {
        self.current_hash = block.successor().unwrap_or_default();
        self.current_height = block.height() + 1;
    }

    fn into_write_details(&self) -> BlockChainSection {
        BlockChainSection {
            account: self.account,
            bottom_height: self.bottom_height,
            bottom_hash: self.bottom_hash,
            top_height: self.top_height,
            top_hash: self.top_hash,
        }
    }
}

#[derive(Default)]
pub(crate) struct CementationWalkerBuilder {
    epochs: Option<Epochs>,
    stopped: Option<Arc<AtomicBool>>,
    max_items: Option<usize>,
}

impl CementationWalkerBuilder {
    pub fn epochs(mut self, epochs: Epochs) -> Self {
        self.epochs = Some(epochs);
        self
    }

    pub fn stopped(mut self, stopped: Arc<AtomicBool>) -> Self {
        self.stopped = Some(stopped);
        self
    }

    pub fn max_items(mut self, max: usize) -> Self {
        self.max_items = Some(max);
        self
    }

    pub fn build(self) -> CementationWalker {
        let epochs = self.epochs.unwrap_or_default();
        let stopped = self
            .stopped
            .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));

        CementationWalker::new(epochs, stopped, self.max_items.unwrap_or(MAX_ITEMS))
    }
}

pub(crate) struct CementationWalker {
    stopped: Arc<AtomicBool>,
    epochs: Epochs,
    chain_stack: BoundedVecDeque<ChainIteration>,
    chains_encountered: usize,
    confirmation_heights: AccountsConfirmedMap,
    original_block: Option<BlockEnum>,
    original_block_hash: BlockHash,
    checkpoints: BoundedVecDeque<BlockHash>,
    latest_cementation: BlockHash,
    block_cache: Arc<BlockCache>,
}

impl CementationWalker {
    pub fn new(epochs: Epochs, stopped: Arc<AtomicBool>, max_items: usize) -> Self {
        Self {
            epochs,
            stopped,
            chain_stack: BoundedVecDeque::new(max_items),
            confirmation_heights: AccountsConfirmedMap::new(),
            chains_encountered: 0,
            original_block: None,
            original_block_hash: BlockHash::zero(),
            checkpoints: BoundedVecDeque::new(max_items),
            latest_cementation: BlockHash::zero(),
            block_cache: Arc::new(BlockCache::new()),
        }
    }

    pub fn builder() -> CementationWalkerBuilder {
        Default::default()
    }

    pub fn block_cache(&self) -> &Arc<BlockCache> {
        &self.block_cache
    }

    pub fn initialize(&mut self, original_block: BlockEnum) {
        self.latest_cementation = BlockHash::zero();
        self.chain_stack.clear();
        self.chains_encountered = 0;
        self.checkpoints.clear();
        self.original_block_hash = original_block.hash();
        self.original_block = Some(original_block);
        self.block_cache.clear();
    }

    pub fn next_cementation<T: LedgerDataRequester>(
        &mut self,
        data_requester: &mut T,
    ) -> Option<BlockChainSection> {
        loop {
            if self.stopped.load(Ordering::Relaxed) {
                return None;
            }
            self.restore_checkpoint_if_required(data_requester);
            let Some(chain) = self.chain_stack.back() else { return None; };

            if chain.is_done() {
                // There is nothing left to do for this chain. We can write the confirmation height now.
                let chain = self.chain_stack.pop_back().unwrap();
                if self.checkpoints.back() == Some(&chain.top_hash) {
                    self.checkpoints.pop_back();
                }
                let new_first_unconfirmed = chain.top_successor;
                if let Some(section) = self.section_to_cement(&chain) {
                    self.cache_confirmation_height(&section, new_first_unconfirmed);
                    self.latest_cementation = section.top_hash;
                    return Some(section);
                }
            } else {
                self.make_sure_all_receive_blocks_have_cemented_send_blocks(
                    chain.search_range(),
                    data_requester,
                );
            }
        }
    }

    fn restore_checkpoint_if_required<T: LedgerDataRequester>(&mut self, data_requester: &mut T) {
        if self.chain_stack.len() > 0 || self.is_done() {
            return; // We still have pending chains. No checkpoint needed.
        }

        let top_hash = self
            .checkpoints
            .pop_back()
            .unwrap_or(self.original_block_hash);

        let block = self.get_block(&top_hash, data_requester);
        self.enqueue_for_cementation(&block, data_requester)
    }

    fn section_to_cement(&self, chain: &ChainIteration) -> Option<BlockChainSection> {
        let mut write_details = chain.into_write_details();
        if let Some(info) = self.confirmation_heights.get(&write_details.account) {
            if info.confirmed_height >= write_details.bottom_height {
                // our bottom is out of date
                if info.confirmed_height >= write_details.top_height {
                    // everything is already cemented
                    return None;
                }
                write_details.bottom_height = info.confirmed_height + 1;
                write_details.bottom_hash = info.first_unconfirmed.unwrap();
            }
        }
        Some(write_details)
    }

    fn cache_confirmation_height(
        &mut self,
        write: &BlockChainSection,
        new_first_unconfirmed: Option<BlockHash>,
    ) {
        self.confirmation_heights.insert(
            write.account,
            ConfirmedInfo {
                confirmed_height: write.top_height,
                confirmed_frontier: write.top_hash,
                first_unconfirmed: new_first_unconfirmed,
            },
        );
    }

    fn make_sure_all_receive_blocks_have_cemented_send_blocks<T: LedgerDataRequester>(
        &mut self,
        search_range: BlockRange,
        data_requester: &mut T,
    ) {
        if let Some((receive, corresponding_send)) =
            self.find_receive_block(&search_range, data_requester)
        {
            let current_chain = self.chain_stack.back_mut().unwrap();
            current_chain.go_to_successor_of(&receive);
            if corresponding_send.account_calculated() != receive.account_calculated() {
                self.enqueue_for_cementation(&corresponding_send, data_requester);
            }
        } else {
            // no more receive blocks in current chain
            self.chain_stack.back_mut().unwrap().set_done();
        }
    }

    fn enqueue_for_cementation<T: LedgerDataRequester>(
        &mut self,
        block: &BlockEnum,
        data_requester: &mut T,
    ) {
        if let Some(lowest) = self.get_lowest_uncemented_block(&block, data_requester) {
            // There are blocks that need to be cemented in this chain
            self.chain_stack
                .push_back(ChainIteration::new(&lowest, &block));
            self.chains_encountered += 1;
            if self.chains_encountered % self.chain_stack.max_len() == 0 {
                // Make a checkpoint every max_len() chains
                self.checkpoints.push_back(block.hash());
            }
        }
    }

    fn get_lowest_uncemented_block<T: LedgerDataRequester>(
        &mut self,
        top_block: &BlockEnum,
        data_requester: &mut T,
    ) -> Option<BlockEnum> {
        let account = top_block.account_calculated();
        match self.get_confirmation_height(&account, data_requester) {
            Some(info) => {
                if top_block.height() <= info.height {
                    None // no uncemented block exists
                } else if top_block.height() - info.height == 1 {
                    Some(top_block.clone()) // top_block is the only uncemented block
                } else if top_block.height() - info.height == 2 {
                    Some(self.get_block(&top_block.previous(), data_requester))
                } else {
                    let frontier_block = self.get_block(&info.frontier, data_requester);
                    self.get_successor_block(&frontier_block, data_requester)
                }
            }
            None => Some(self.get_open_block(&account, data_requester)),
        }
    }

    fn get_confirmation_height<T: LedgerDataRequester>(
        &self,
        account: &Account,
        data_requester: &T,
    ) -> Option<ConfirmationHeightInfo> {
        match self.confirmation_heights.get(account) {
            Some(info) => Some(ConfirmationHeightInfo {
                height: info.confirmed_height,
                frontier: info.confirmed_frontier,
            }),
            None => data_requester.get_confirmation_height(account),
        }
    }

    fn find_receive_block<T: LedgerDataRequester>(
        &mut self,
        range: &BlockRange,
        data_requester: &mut T,
    ) -> Option<(BlockEnum, BlockEnum)> {
        let mut current = self.get_block(&range.bottom, data_requester);
        loop {
            if let Some(send) = self.get_corresponding_send_block(&current, data_requester) {
                return Some((current, send));
            }

            if current.hash() == range.top || self.stopped.load(Ordering::Relaxed) {
                return None;
            }

            current = self
                .get_successor_block(&current, data_requester)
                .expect("invalid block range given");
        }
    }

    pub fn is_accounts_cache_full(&self) -> bool {
        self.confirmation_heights.len() >= self.chain_stack.max_len()
    }

    pub fn is_done(&self) -> bool {
        self.latest_cementation == self.original_block_hash || self.stopped.load(Ordering::Relaxed)
    }

    fn get_corresponding_send_block<T: LedgerDataRequester>(
        &mut self,
        block: &BlockEnum,
        data_requester: &mut T,
    ) -> Option<BlockEnum> {
        let source = block.source_or_link();
        if !source.is_zero() && !self.epochs.is_epoch_link(&source.into()) {
            self.block_cache.load_block(&source, data_requester)
        } else {
            None
        }
    }

    pub fn clear_all_cached_accounts(&mut self) {
        self.confirmation_heights.clear();
    }

    pub fn section_cemented(&mut self, account: &Account, height: u64) {
        if let Some(found_info) = self.confirmation_heights.get(account) {
            if found_info.confirmed_height == height {
                self.confirmation_heights.remove(account);
            }
        }
    }

    pub(crate) fn container_info(&self) -> AccountsConfirmedMapContainerInfo {
        self.confirmation_heights.container_info()
    }

    fn get_successor_block<T: LedgerDataRequester>(
        &mut self,
        block: &BlockEnum,
        data_requester: &mut T,
    ) -> Option<BlockEnum> {
        block
            .successor()
            .map(|successor| self.get_block(&successor, data_requester))
    }

    fn get_open_block<T: LedgerDataRequester>(
        &mut self,
        account: &Account,
        data_requester: &mut T,
    ) -> BlockEnum {
        let open_hash = data_requester
            .get_account_info(account)
            .expect("could not load account info")
            .open_block;

        self.get_block(&open_hash, data_requester)
    }

    fn get_block<T: LedgerDataRequester>(
        &mut self,
        block_hash: &BlockHash,
        data_requester: &mut T,
    ) -> BlockEnum {
        if *block_hash == self.original_block_hash {
            return self.original_block.as_ref().unwrap().clone();
        }
        self.block_cache
            .load_block(block_hash, data_requester)
            .expect("could not load block")
    }

    pub(crate) fn notify_block_already_cemented(&self, callback: &mut dyn FnMut(BlockHash)) {
        if self.chains_encountered == 0 {
            callback(self.original_block_hash);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::cementation::LedgerDataRequesterStub;
    use rsnano_core::{Amount, TestAccountChain};

    #[test]
    fn block_not_found() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut sut = CementationWalker::builder().build();
        let mut genesis_chain = data_requester.add_genesis_block();
        genesis_chain.legacy_send();
        genesis_chain.legacy_send();
        sut.initialize(genesis_chain.latest_block().clone());

        let result = std::panic::catch_unwind(move || sut.next_cementation(&mut data_requester));
        assert!(result.is_err());
    }

    #[test]
    fn stopped() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let stopped = Arc::new(AtomicBool::new(false));
        let mut sut = CementationWalker::builder()
            .stopped(stopped.clone())
            .build();

        let mut genesis_chain = data_requester.add_genesis_block();
        genesis_chain.legacy_send();
        data_requester.add_uncemented(&genesis_chain);
        sut.initialize(genesis_chain.latest_block().clone());

        stopped.store(true, Ordering::Relaxed);

        let step = sut.next_cementation(&mut data_requester);
        assert_eq!(step, None)
    }

    #[test]
    fn cement_first_send_from_genesis() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut genesis_chain = data_requester.add_genesis_block();
        genesis_chain.legacy_send();
        data_requester.add_uncemented(&genesis_chain);

        assert_write_steps(
            &mut data_requester,
            genesis_chain.latest_block().clone(),
            &[genesis_chain.frontier_section()],
        );

        assert_eq!(data_requester.blocks_loaded(), 0);
        assert_eq!(data_requester.confirmation_heights_loaded(), 1);
    }
    #[test]
    fn cement_two_blocks_in_one_go() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut genesis_chain = data_requester.add_genesis_block();
        genesis_chain.legacy_send();
        genesis_chain.legacy_send();
        let second_send = genesis_chain.block(3).clone();
        data_requester.add_uncemented(&genesis_chain);

        assert_write_steps(
            &mut data_requester,
            second_send,
            &[genesis_chain.section(2, 3)],
        );
        assert_eq!(data_requester.blocks_loaded(), 1);
        assert_eq!(data_requester.confirmation_heights_loaded(), 1);
    }

    #[test]
    fn cement_three_blocks_in_one_go() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut genesis_chain = data_requester.add_genesis_block();
        genesis_chain.legacy_send();
        genesis_chain.legacy_send();
        genesis_chain.legacy_send();
        data_requester.add_uncemented(&genesis_chain);

        assert_write_steps(
            &mut data_requester,
            genesis_chain.latest_block().clone(),
            &[genesis_chain.section(2, 4)],
        );
        assert_eq!(data_requester.blocks_loaded(), 3);
        assert_eq!(data_requester.confirmation_heights_loaded(), 1);
    }

    #[test]
    fn cement_open_block() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut dest_chain = TestAccountChain::new();
        let mut genesis_chain = data_requester.add_genesis_block();
        genesis_chain.legacy_send_to(dest_chain.account(), Amount::raw(10));
        dest_chain.legacy_open_from_account(&genesis_chain);
        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&dest_chain);

        assert_write_steps(
            &mut data_requester,
            dest_chain.latest_block().clone(),
            &[dest_chain.frontier_section()],
        );
    }

    #[test]
    fn cement_open_block_and_successor_in_one_go() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut genesis_chain = data_requester.add_genesis_block();
        genesis_chain.legacy_send();
        let mut dest_chain = TestAccountChain::new();
        dest_chain.legacy_open_from_account(&genesis_chain);
        dest_chain.legacy_send();
        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&dest_chain);

        assert_write_steps(
            &mut data_requester,
            dest_chain.latest_block().clone(),
            &[dest_chain.section(1, 2)],
        );
    }

    #[test]
    fn cement_open_block_and_two_successors_in_one_go() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut dest_chain = TestAccountChain::new();
        let mut genesis_chain = data_requester.add_genesis_block();
        genesis_chain.legacy_send_to(dest_chain.account(), Amount::raw(2));
        dest_chain.legacy_open_from_account(&genesis_chain);
        dest_chain.legacy_send();
        dest_chain.legacy_send();
        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&dest_chain);

        assert_write_steps(
            &mut data_requester,
            dest_chain.latest_block().clone(),
            &[dest_chain.section(1, 3)],
        );
    }

    #[test]
    fn cement_receive_block() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut dest_chain = TestAccountChain::new();
        let mut genesis_chain = data_requester.add_genesis_block();
        genesis_chain.legacy_send_to(dest_chain.account(), Amount::raw(1));
        genesis_chain.legacy_send_to(dest_chain.account(), Amount::raw(1));
        dest_chain.legacy_open_from_account_block(&genesis_chain, 2);
        data_requester.add_cemented(&genesis_chain);
        data_requester.add_cemented(&dest_chain);

        dest_chain.legacy_receive_from_account(&genesis_chain);
        data_requester.add_uncemented(&dest_chain);

        assert_write_steps(
            &mut data_requester,
            dest_chain.latest_block().clone(),
            &[dest_chain.frontier_section()],
        );
    }
    #[test]
    fn cement_two_accounts_in_one_go() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut genesis_chain = data_requester.add_genesis_block();
        let mut dest_1 = TestAccountChain::new();
        let mut dest_2 = TestAccountChain::new();
        genesis_chain.legacy_send_to(dest_1.account(), Amount::raw(5));

        dest_1.legacy_open_from_account(&genesis_chain);
        dest_1.legacy_send();
        dest_1.legacy_send();
        dest_1.legacy_send_to(dest_2.account(), Amount::raw(3));

        dest_2.legacy_open_from_account(&dest_1);
        dest_2.legacy_send();
        dest_2.legacy_send();
        dest_2.legacy_send();

        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&dest_1);
        data_requester.add_uncemented(&dest_2);

        assert_write_steps(
            &mut data_requester,
            dest_2.latest_block().clone(),
            &[dest_1.section(1, 4), dest_2.section(1, 4)],
        );
    }

    #[test]
    fn send_to_self() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut chain = data_requester.add_genesis_block();
        let account = chain.account();
        chain.legacy_send_to(account, Amount::raw(10));
        chain.legacy_receive_from_self();
        data_requester.add_uncemented(&chain);

        assert_write_steps(
            &mut data_requester,
            chain.latest_block().clone(),
            &[chain.section(2, 3)],
        );
    }

    #[test]
    fn receive_and_send() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut dest_chain = TestAccountChain::new();
        let mut genesis_chain = data_requester.add_genesis_block();
        genesis_chain.legacy_send_to(dest_chain.account(), Amount::raw(1));
        dest_chain.legacy_open_from_account(&genesis_chain);
        dest_chain.legacy_send();
        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&dest_chain);

        assert_write_steps(
            &mut data_requester,
            dest_chain.latest_block().clone(),
            &[dest_chain.section(1, 2)],
        );
    }

    #[test]
    fn complex_example() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut account1 = TestAccountChain::new();
        let mut account2 = TestAccountChain::new();
        let mut account3 = TestAccountChain::new();
        let mut genesis_chain = data_requester.add_genesis_block();
        genesis_chain.legacy_send_to(account1.account(), Amount::raw(5));

        account1.legacy_open_from_account(&genesis_chain);
        account1.legacy_send();
        account1.legacy_send_to(account2.account(), Amount::raw(4));

        account2.legacy_open_from_account(&account1);
        account2.legacy_send_to(account3.account(), Amount::raw(4));

        account3.legacy_open_from_account(&account2);
        account3.legacy_send();
        account3.legacy_send_to(account1.account(), Amount::raw(3));

        account1.legacy_receive_from_account(&account3);
        account1.legacy_send();
        account1.legacy_send_to(account2.account(), Amount::raw(1));

        account2.legacy_receive_from_account(&account1);

        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&account1);
        data_requester.add_uncemented(&account2);
        data_requester.add_uncemented(&account3);

        assert_write_steps(
            &mut data_requester,
            account2.latest_block().clone(),
            &[
                account1.section(1, 3),
                account2.section(1, 2),
                account3.section(1, 3),
                account1.section(4, 6),
                account2.section(3, 3),
            ],
        );
        assert_eq!(data_requester.blocks_loaded(), 12);
        assert_eq!(data_requester.confirmation_heights_loaded(), 5);
    }

    #[test]
    fn block_already_cemented() {
        let mut sut = CementationWalker::builder().build();
        let mut data_requester = LedgerDataRequesterStub::new();
        let genesis_chain = data_requester.add_genesis_block();

        sut.initialize(genesis_chain.latest_block().clone());
        let step = sut.next_cementation(&mut data_requester);

        assert_eq!(step, None);
    }

    #[test]
    fn use_checkpoints() {
        let mut data_requester = LedgerDataRequesterStub::new();
        let mut account1 = TestAccountChain::new();
        let mut account2 = TestAccountChain::new();
        let mut account3 = TestAccountChain::new();
        let mut account4 = TestAccountChain::new();
        let mut account5 = TestAccountChain::new();
        let mut account6 = TestAccountChain::new();
        let mut genesis_chain = data_requester.add_genesis_block();
        genesis_chain.legacy_send_to(account1.account(), Amount::raw(1));

        account1.legacy_open_from_account(&genesis_chain);
        account1.legacy_send_to(account2.account(), Amount::raw(1));

        account2.legacy_open_from_account(&account1);
        account2.legacy_send_to(account3.account(), Amount::raw(1));

        account3.legacy_open_from_account(&account2);
        account3.legacy_send_to(account4.account(), Amount::raw(1));

        account4.legacy_open_from_account(&account3);
        account4.legacy_send_to(account5.account(), Amount::raw(1));

        account5.legacy_open_from_account(&account4);
        account5.legacy_send_to(account6.account(), Amount::raw(1));

        account6.legacy_open_from_account(&account5);
        account6.legacy_send();

        data_requester.add_cemented(&genesis_chain);
        data_requester.add_uncemented(&account1);
        data_requester.add_uncemented(&account2);
        data_requester.add_uncemented(&account3);
        data_requester.add_uncemented(&account4);
        data_requester.add_uncemented(&account5);
        data_requester.add_uncemented(&account6);

        assert_write_steps_with_max_items(
            2,
            &mut data_requester,
            account6.latest_block().clone(),
            &[
                account1.section(1, 2),
                account2.section(1, 2),
                account3.section(1, 2),
                account4.section(1, 2),
                account5.section(1, 2),
                account6.section(1, 2),
            ],
        );

        assert_eq!(data_requester.blocks_loaded(), 12);
        assert_eq!(data_requester.confirmation_heights_loaded(), 12);
    }

    mod pruning {
        use super::*;

        #[test]
        fn send_block_pruned() {
            let mut data_requester = LedgerDataRequesterStub::new();
            let mut dest_chain = TestAccountChain::new();
            let mut genesis_chain = data_requester.add_genesis_block();
            genesis_chain.legacy_send_to(dest_chain.account(), Amount::raw(1));
            dest_chain.legacy_open_from_account(&genesis_chain);
            data_requester.add_cemented(&genesis_chain);
            data_requester.add_uncemented(&dest_chain);
            data_requester.prune(genesis_chain.frontier());

            assert_write_steps(
                &mut data_requester,
                dest_chain.latest_block().clone(),
                &[dest_chain.frontier_section()],
            );
        }
    }

    fn assert_write_steps(
        data_requester: &mut LedgerDataRequesterStub,
        block_to_cement: BlockEnum,
        expected: &[BlockChainSection],
    ) {
        assert_write_steps_with_max_items(MAX_ITEMS, data_requester, block_to_cement, expected)
    }

    fn assert_write_steps_with_max_items(
        max_items: usize,
        data_requester: &mut LedgerDataRequesterStub,
        block_to_cement: BlockEnum,
        expected: &[BlockChainSection],
    ) {
        let mut sut = CementationWalker::builder().max_items(max_items).build();
        sut.initialize(block_to_cement);

        let mut actual = Vec::new();
        while let Some(section) = sut.next_cementation(data_requester) {
            actual.push(section);
        }

        for (i, (act, exp)) in actual.iter().zip(expected).enumerate() {
            assert_eq!(act, exp, "Unexpected WriteDetails at index {}", i);
        }

        if actual.len() < expected.len() {
            panic!(
                "actual as too few elements. These are missing: {:?}",
                &expected[actual.len()..]
            );
        }

        if actual.len() > expected.len() {
            panic!(
                "actual as too many elements. These are too many: {:?}",
                &actual[expected.len()..]
            );
        }

        assert_eq!(sut.checkpoints.len(), 0);
    }
}
