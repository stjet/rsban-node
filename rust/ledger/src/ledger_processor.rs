use crate::LedgerConstants;
use rsnano_core::{
    utils::seconds_since_epoch, validate_block_signature, validate_message, AccountInfo, Amount,
    Block, BlockDetails, BlockHash, BlockSideband, BlockSubType, ChangeBlock, Epoch, Epochs,
    MutableBlockVisitor, OpenBlock, PendingInfo, PendingKey, ReceiveBlock, SendBlock, StateBlock,
};
use rsnano_store_traits::WriteTransaction;

use super::{Ledger, LedgerObserver, ProcessResult};

pub(crate) struct LedgerProcessor<'a> {
    ledger: &'a Ledger,
    observer: &'a dyn LedgerObserver,
    constants: &'a LedgerConstants,
    txn: &'a mut dyn WriteTransaction,
    pub result: ProcessResult,
}

impl<'a> LedgerProcessor<'a> {
    pub(crate) fn new(
        ledger: &'a Ledger,
        observer: &'a dyn LedgerObserver,
        constants: &'a LedgerConstants,
        txn: &'a mut dyn WriteTransaction,
    ) -> Self {
        Self {
            ledger,
            observer,
            constants,
            txn,
            result: ProcessResult::Progress,
        }
    }
    // Returns true if this block which has an epoch link is correctly formed.
    fn validate_epoch_block(&mut self, block: &StateBlock) -> bool {
        debug_assert!(self.ledger.is_epoch_link(&block.link()));
        let mut prev_balance = Amount::zero();
        self.result = if !block.previous().is_zero() {
            if self
                .ledger
                .store
                .block()
                .exists(self.txn.txn(), &block.previous())
            {
                prev_balance = self.ledger.balance(self.txn.txn(), &block.previous());
                ProcessResult::Progress
            } else {
                // Check for possible regular state blocks with epoch link (send subtype)
                if validate_block_signature(block).is_err()
                    && self.ledger.validate_epoch_signature(block).is_err()
                {
                    ProcessResult::BadSignature
                } else {
                    ProcessResult::GapPrevious
                }
            }
        } else {
            ProcessResult::Progress
        };
        block.balance() == prev_balance
    }

    fn epoch_block_impl(&mut self, block: &mut StateBlock) {
        let hash = block.hash();
        let existing = self
            .ledger
            .block_or_pruned_exists_txn(self.txn.txn(), &hash);
        // Have we seen this block before? (Unambiguous)
        self.result = if existing {
            ProcessResult::Old
        } else {
            ProcessResult::Progress
        };
        if self.result == ProcessResult::Progress {
            // Is this block signed correctly (Unambiguous)
            self.result = match validate_message(
                &self
                    .ledger
                    .epoch_signer(&block.link())
                    .unwrap_or_default()
                    .into(),
                hash.as_bytes(),
                block.block_signature(),
            ) {
                Ok(_) => ProcessResult::Progress,
                Err(_) => ProcessResult::BadSignature,
            };
            if self.result == ProcessResult::Progress {
                debug_assert!(validate_message(
                    &self
                        .ledger
                        .epoch_signer(&block.link())
                        .unwrap_or_default()
                        .into(),
                    hash.as_bytes(),
                    block.block_signature()
                )
                .is_ok());
                // Is this for the burn account? (Unambiguous)
                self.result = if block.account().is_zero() {
                    ProcessResult::OpenedBurnAccount
                } else {
                    ProcessResult::Progress
                };
                if self.result == ProcessResult::Progress {
                    let mut info = AccountInfo::default();
                    let mut account_error = false;
                    match self
                        .ledger
                        .store
                        .account()
                        .get(self.txn.txn(), &block.account())
                    {
                        Some(i) => {
                            // Account already exists
                            info = i;
                            // Has this account already been opened? (Ambigious)
                            self.result = if block.previous().is_zero() {
                                ProcessResult::Fork
                            } else {
                                ProcessResult::Progress
                            };
                            if self.result == ProcessResult::Progress {
                                // Is the previous block the account's head block? (Ambigious)
                                self.result = if block.previous() == info.head {
                                    ProcessResult::Progress
                                } else {
                                    ProcessResult::Fork
                                };
                                if self.result == ProcessResult::Progress {
                                    self.result = if block.representative() == info.representative {
                                        ProcessResult::Progress
                                    } else {
                                        ProcessResult::RepresentativeMismatch
                                    };
                                }
                            }
                        }
                        None => {
                            account_error = true;
                            self.result = if block.representative().is_zero() {
                                ProcessResult::Progress
                            } else {
                                ProcessResult::RepresentativeMismatch
                            };
                            // Non-exisitng account should have pending entries
                            if self.result == ProcessResult::Progress {
                                let pending_exists = self
                                    .ledger
                                    .store
                                    .pending()
                                    .any(self.txn.txn(), &block.account());
                                self.result = if pending_exists {
                                    ProcessResult::Progress
                                } else {
                                    ProcessResult::GapEpochOpenPending
                                };
                            }
                        }
                    }

                    if self.result == ProcessResult::Progress {
                        let epoch = self
                            .constants
                            .epochs
                            .epoch(&block.link())
                            .unwrap_or(Epoch::Invalid);
                        // Must be an epoch for an unopened account or the epoch upgrade must be sequential
                        let is_valid_epoch_upgrade = if account_error {
                            epoch != Epoch::Invalid
                        } else {
                            Epochs::is_sequential(info.epoch, epoch)
                        };
                        self.result = if is_valid_epoch_upgrade {
                            ProcessResult::Progress
                        } else {
                            ProcessResult::BlockPosition
                        };
                        if self.result == ProcessResult::Progress {
                            self.result = if block.balance() == info.balance {
                                ProcessResult::Progress
                            } else {
                                ProcessResult::BalanceMismatch
                            };
                            if self.result == ProcessResult::Progress {
                                let block_details = BlockDetails::new(epoch, false, false, true);
                                // Does this block have sufficient work (Malformed)
                                self.result = if self.constants.work.difficulty_block(block)
                                    >= self
                                        .constants
                                        .work
                                        .threshold2(block.work_version(), &block_details)
                                {
                                    ProcessResult::Progress
                                } else {
                                    ProcessResult::InsufficientWork
                                };
                                if self.result == ProcessResult::Progress {
                                    self.observer.block_added(BlockSubType::Epoch);
                                    block.set_sideband(BlockSideband::new(
                                        block.account(), /* unused */
                                        BlockHash::zero(),
                                        Amount::zero(), /* unused */
                                        info.block_count + 1,
                                        seconds_since_epoch(),
                                        block_details,
                                        Epoch::Epoch0, /* unused */
                                    ));
                                    self.ledger.store.block().put(self.txn, &hash, block);
                                    let new_info = AccountInfo {
                                        head: hash,
                                        representative: block.representative(),
                                        open_block: if info.open_block.is_zero() {
                                            hash
                                        } else {
                                            info.open_block
                                        },
                                        balance: info.balance,
                                        modified: seconds_since_epoch(),
                                        block_count: info.block_count + 1,
                                        epoch,
                                    };
                                    self.ledger.update_account(
                                        self.txn,
                                        &block.account(),
                                        &info,
                                        &new_info,
                                    );
                                    if self
                                        .ledger
                                        .store
                                        .frontier()
                                        .get(self.txn.txn(), &info.head)
                                        .is_some()
                                    {
                                        self.ledger.store.frontier().del(self.txn, &info.head);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl<'a> MutableBlockVisitor for LedgerProcessor<'a> {
    fn send_block(&mut self, block: &mut SendBlock) {
        let hash = block.hash();
        let existing = self
            .ledger
            .block_or_pruned_exists_txn(self.txn.txn(), &hash);
        self.result = if existing {
            ProcessResult::Old
        } else {
            ProcessResult::Progress
        }; // Have we seen this block before? (Harmless)
        if self.result == ProcessResult::Progress {
            let previous = self
                .ledger
                .store
                .block()
                .get(self.txn.txn(), &block.previous());
            // Have we seen the previous block already? (Harmless)
            let previous = match previous {
                Some(b) => b,
                None => {
                    self.result = ProcessResult::GapPrevious;
                    return;
                }
            };
            self.result = if SendBlock::valid_predecessor(previous.block_type()) {
                ProcessResult::Progress
            } else {
                ProcessResult::BlockPosition
            };
            if self.result == ProcessResult::Progress {
                let account = self.ledger.get_frontier(self.txn.txn(), &block.previous());
                self.result = if account.is_none() {
                    ProcessResult::Fork
                } else {
                    ProcessResult::Progress
                };
                if self.result == ProcessResult::Progress {
                    let account = account.unwrap();
                    // Is this block signed correctly (Malformed)
                    self.result = match validate_message(
                        &account.into(),
                        hash.as_bytes(),
                        block.block_signature(),
                    ) {
                        Ok(_) => ProcessResult::Progress,
                        Err(_) => ProcessResult::BadSignature,
                    };
                    if self.result == ProcessResult::Progress {
                        let block_details = BlockDetails::new(
                            Epoch::Epoch0,
                            false, /* unused */
                            false, /* unused */
                            false, /* unused */
                        );
                        // Does this block have sufficient work? (Malformed)
                        self.result = if self.constants.work.difficulty_block(block)
                            >= self
                                .constants
                                .work
                                .threshold2(block.work_version(), &block_details)
                        {
                            ProcessResult::Progress
                        } else {
                            ProcessResult::InsufficientWork
                        };
                        if self.result == ProcessResult::Progress {
                            debug_assert!(validate_message(
                                &account.into(),
                                hash.as_bytes(),
                                block.block_signature()
                            )
                            .is_ok());
                            let (info, latest_error) =
                                match self.ledger.store.account().get(self.txn.txn(), &account) {
                                    Some(i) => (i, false),
                                    None => (AccountInfo::default(), true),
                                };
                            debug_assert!(!latest_error);
                            debug_assert!(info.head == block.previous());
                            // Is this trying to spend a negative amount (Malicious)
                            self.result = if info.balance >= block.balance() {
                                ProcessResult::Progress
                            } else {
                                ProcessResult::NegativeSpend
                            };
                            if self.result == ProcessResult::Progress {
                                let amount = info.balance - block.balance();
                                self.ledger.cache.rep_weights.representation_add(
                                    info.representative,
                                    Amount::zero().wrapping_sub(amount),
                                );
                                block.set_sideband(BlockSideband::new(
                                    account,
                                    BlockHash::zero(),
                                    block.balance(), /* unused */
                                    info.block_count + 1,
                                    seconds_since_epoch(),
                                    block_details,
                                    Epoch::Epoch0, /* unused */
                                ));
                                self.ledger.store.block().put(self.txn, &hash, block);
                                let new_info = AccountInfo {
                                    head: hash,
                                    representative: info.representative,
                                    open_block: info.open_block,
                                    balance: block.balance(),
                                    modified: seconds_since_epoch(),
                                    block_count: info.block_count + 1,
                                    epoch: Epoch::Epoch0,
                                };
                                self.ledger
                                    .update_account(self.txn, &account, &info, &new_info);
                                self.ledger.store.pending().put(
                                    self.txn,
                                    &PendingKey::new(block.hashables.destination, hash),
                                    &PendingInfo::new(account, amount, Epoch::Epoch0),
                                );
                                self.ledger
                                    .store
                                    .frontier()
                                    .del(self.txn, &block.previous());
                                self.ledger.store.frontier().put(self.txn, &hash, &account);
                                self.observer.block_added(BlockSubType::Send);
                            }
                        }
                    }
                }
            }
        }
    }

    fn receive_block(&mut self, block: &mut ReceiveBlock) {
        let hash = block.hash();
        let existing = self
            .ledger
            .block_or_pruned_exists_txn(self.txn.txn(), &hash);
        // Have we seen this block already?  (Harmless)
        self.result = if existing {
            ProcessResult::Old
        } else {
            ProcessResult::Progress
        };
        if self.result == ProcessResult::Progress {
            let previous = self
                .ledger
                .store
                .block()
                .get(self.txn.txn(), &block.previous());
            let previous = match previous {
                Some(b) => b,
                None => {
                    self.result = ProcessResult::GapPrevious;
                    return;
                }
            };
            self.result = if ReceiveBlock::valid_predecessor(previous.block_type()) {
                ProcessResult::Progress
            } else {
                ProcessResult::BlockPosition
            };
            if self.result == ProcessResult::Progress {
                let account = self.ledger.get_frontier(self.txn.txn(), &block.previous());
                // Have we seen the previous block? No entries for account at all (Harmless)
                self.result = if account.is_none() {
                    ProcessResult::GapPrevious
                } else {
                    ProcessResult::Progress
                };
                if self.result == ProcessResult::Progress {
                    let account = account.unwrap();
                    // Is the signature valid (Malformed)
                    self.result = match validate_message(
                        &account.into(),
                        hash.as_bytes(),
                        block.block_signature(),
                    ) {
                        Ok(_) => ProcessResult::Progress,
                        Err(_) => ProcessResult::BadSignature,
                    };
                    if self.result == ProcessResult::Progress {
                        debug_assert!(validate_message(
                            &account.into(),
                            hash.as_bytes(),
                            block.block_signature()
                        )
                        .is_ok());
                        // Have we seen the source block already? (Harmless)
                        self.result = if self
                            .ledger
                            .block_or_pruned_exists_txn(self.txn.txn(), &block.source())
                        {
                            ProcessResult::Progress
                        } else {
                            ProcessResult::GapSource
                        };
                        if self.result == ProcessResult::Progress {
                            let info = self
                                .ledger
                                .store
                                .account()
                                .get(self.txn.txn(), &account)
                                .unwrap_or_default();
                            // Block doesn't immediately follow latest block (Harmless)
                            self.result = if info.head == block.previous() {
                                ProcessResult::Progress
                            } else {
                                ProcessResult::GapPrevious
                            };
                            if self.result == ProcessResult::Progress {
                                let key = PendingKey::new(account, block.source());
                                // Has this source already been received (Malformed)
                                let pending =
                                    match self.ledger.store.pending().get(self.txn.txn(), &key) {
                                        Some(i) => i,
                                        None => {
                                            self.result = ProcessResult::Unreceivable;
                                            PendingInfo::default()
                                        }
                                    };
                                if self.result == ProcessResult::Progress {
                                    // Are we receiving a state-only send? (Malformed)
                                    self.result = if pending.epoch == Epoch::Epoch0 {
                                        ProcessResult::Progress
                                    } else {
                                        ProcessResult::Unreceivable
                                    };
                                    if self.result == ProcessResult::Progress {
                                        let block_details = BlockDetails::new(
                                            Epoch::Epoch0,
                                            false, /* unused */
                                            false, /* unused */
                                            false, /* unused */
                                        );
                                        // Does this block have sufficient work? (Malformed)
                                        self.result = if self.constants.work.difficulty_block(block)
                                            >= self
                                                .constants
                                                .work
                                                .threshold2(block.work_version(), &block_details)
                                        {
                                            ProcessResult::Progress
                                        } else {
                                            ProcessResult::InsufficientWork
                                        };
                                        if self.result == ProcessResult::Progress {
                                            let new_balance = info.balance + pending.amount;
                                            #[cfg(debug_assertions)]
                                            {
                                                if self
                                                    .ledger
                                                    .store
                                                    .block()
                                                    .exists(self.txn.txn(), &block.source())
                                                {
                                                    let source_info = self
                                                        .ledger
                                                        .store
                                                        .account()
                                                        .get(self.txn.txn(), &pending.source);
                                                    debug_assert!(source_info.is_some());
                                                }
                                            }
                                            self.ledger.store.pending().del(self.txn, &key);
                                            block.set_sideband(BlockSideband::new(
                                                account,
                                                BlockHash::zero(),
                                                new_balance,
                                                info.block_count + 1,
                                                seconds_since_epoch(),
                                                block_details,
                                                Epoch::Epoch0, /* unused */
                                            ));
                                            self.ledger.store.block().put(self.txn, &hash, block);
                                            let new_info = AccountInfo {
                                                head: hash,
                                                representative: info.representative,
                                                open_block: info.open_block,
                                                balance: new_balance,
                                                modified: seconds_since_epoch(),
                                                block_count: info.block_count + 1,
                                                epoch: Epoch::Epoch0,
                                            };
                                            self.ledger.update_account(
                                                self.txn, &account, &info, &new_info,
                                            );
                                            self.ledger.cache.rep_weights.representation_add(
                                                info.representative,
                                                pending.amount,
                                            );
                                            self.ledger
                                                .store
                                                .frontier()
                                                .del(self.txn, &block.previous());
                                            self.ledger
                                                .store
                                                .frontier()
                                                .put(self.txn, &hash, &account);
                                            self.observer.block_added(BlockSubType::Receive);
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // If we have the block but it's not the latest we have a signed fork (Malicious)
                    self.result = if self
                        .ledger
                        .store
                        .block()
                        .exists(self.txn.txn(), &block.previous())
                    {
                        ProcessResult::Fork
                    } else {
                        ProcessResult::GapPrevious
                    };
                }
            }
        }
    }

    fn open_block(&mut self, block: &mut OpenBlock) {
        let hash = block.hash();
        let existing = self
            .ledger
            .block_or_pruned_exists_txn(self.txn.txn(), &hash);
        // Have we seen this block already? (Harmless)
        self.result = if existing {
            ProcessResult::Old
        } else {
            ProcessResult::Progress
        };
        if self.result == ProcessResult::Progress {
            // Is the signature valid (Malformed)
            self.result = match validate_message(
                &block.account().into(),
                hash.as_bytes(),
                block.block_signature(),
            ) {
                Ok(_) => ProcessResult::Progress,
                Err(_) => ProcessResult::BadSignature,
            };
            if self.result == ProcessResult::Progress {
                debug_assert!(validate_message(
                    &block.account().into(),
                    hash.as_bytes(),
                    block.block_signature()
                )
                .is_ok());
                // Have we seen the source block? (Harmless)
                self.result = if self
                    .ledger
                    .block_or_pruned_exists_txn(self.txn.txn(), &block.source())
                {
                    ProcessResult::Progress
                } else {
                    ProcessResult::GapSource
                };
                if self.result == ProcessResult::Progress {
                    // Has this account already been opened? (Malicious)
                    self.result = match self
                        .ledger
                        .store
                        .account()
                        .get(self.txn.txn(), &block.account())
                    {
                        Some(_) => ProcessResult::Fork,
                        None => ProcessResult::Progress,
                    };
                    if self.result == ProcessResult::Progress {
                        let key = PendingKey::new(block.account(), block.source());
                        // Has this source already been received (Malformed)
                        let pending = match self.ledger.store.pending().get(self.txn.txn(), &key) {
                            Some(i) => i,
                            None => {
                                self.result = ProcessResult::Unreceivable;
                                PendingInfo::default()
                            }
                        };
                        if self.result == ProcessResult::Progress {
                            // Is it burning 0 account? (Malicious)
                            self.result = if block.account() == self.constants.burn_account {
                                ProcessResult::OpenedBurnAccount
                            } else {
                                ProcessResult::Progress
                            };
                            if self.result == ProcessResult::Progress {
                                // Are we receiving a state-only send? (Malformed)
                                self.result = if pending.epoch == Epoch::Epoch0 {
                                    ProcessResult::Progress
                                } else {
                                    ProcessResult::Unreceivable
                                };
                                if self.result == ProcessResult::Progress {
                                    let block_details = BlockDetails::new(
                                        Epoch::Epoch0,
                                        false, /* unused */
                                        false, /* unused */
                                        false, /* unused */
                                    );
                                    // Does this block have sufficient work? (Malformed)
                                    self.result = if self.constants.work.difficulty_block(block)
                                        >= self
                                            .constants
                                            .work
                                            .threshold2(block.work_version(), &block_details)
                                    {
                                        ProcessResult::Progress
                                    } else {
                                        ProcessResult::InsufficientWork
                                    };
                                    if self.result == ProcessResult::Progress {
                                        #[cfg(debug_assertions)]
                                        {
                                            if self
                                                .ledger
                                                .store
                                                .block()
                                                .exists(self.txn.txn(), &block.source())
                                            {
                                                let source_info = self
                                                    .ledger
                                                    .store
                                                    .account()
                                                    .get(self.txn.txn(), &pending.source);
                                                debug_assert!(source_info.is_some());
                                            }
                                        }
                                        self.ledger.store.pending().del(self.txn, &key);
                                        block.set_sideband(BlockSideband::new(
                                            block.account(),
                                            BlockHash::zero(),
                                            pending.amount,
                                            1,
                                            seconds_since_epoch(),
                                            block_details,
                                            Epoch::Epoch0, /* unused */
                                        ));
                                        self.ledger.store.block().put(self.txn, &hash, block);
                                        let new_info = AccountInfo {
                                            head: hash,
                                            representative: block.representative(),
                                            open_block: hash,
                                            balance: pending.amount,
                                            modified: seconds_since_epoch(),
                                            block_count: 1,
                                            epoch: Epoch::Epoch0,
                                        };
                                        self.ledger.update_account(
                                            self.txn,
                                            &block.account(),
                                            &AccountInfo::default(),
                                            &new_info,
                                        );
                                        self.ledger.cache.rep_weights.representation_add(
                                            block.representative(),
                                            pending.amount,
                                        );
                                        self.ledger.store.frontier().put(
                                            self.txn,
                                            &hash,
                                            &block.account(),
                                        );
                                        self.observer.block_added(BlockSubType::Open);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn change_block(&mut self, block: &mut ChangeBlock) {
        let hash = block.hash();
        let existing = self
            .ledger
            .block_or_pruned_exists_txn(self.txn.txn(), &hash);
        // Have we seen this block before? (Harmless)
        self.result = if existing {
            ProcessResult::Old
        } else {
            ProcessResult::Progress
        };
        if self.result == ProcessResult::Progress {
            // Have we seen the previous block already? (Harmless)
            let previous = match self
                .ledger
                .store
                .block()
                .get(self.txn.txn(), &block.previous())
            {
                Some(b) => b,
                None => {
                    self.result = ProcessResult::GapPrevious;
                    return;
                }
            };
            self.result = if ChangeBlock::valid_predecessor(previous.block_type()) {
                ProcessResult::Progress
            } else {
                ProcessResult::BlockPosition
            };
            if self.result == ProcessResult::Progress {
                let account = self
                    .ledger
                    .store
                    .frontier()
                    .get(self.txn.txn(), &block.previous());
                self.result = if account.is_none() {
                    ProcessResult::Fork
                } else {
                    ProcessResult::Progress
                };
                if self.result == ProcessResult::Progress {
                    let account = account.unwrap();
                    let (info, latest_error) =
                        match self.ledger.store.account().get(self.txn.txn(), &account) {
                            Some(i) => (i, false),
                            None => (AccountInfo::default(), true),
                        };
                    debug_assert!(!latest_error);
                    debug_assert!(info.head == block.previous());
                    // Is this block signed correctly (Malformed)
                    self.result = match validate_message(
                        &account.into(),
                        hash.as_bytes(),
                        block.block_signature(),
                    ) {
                        Ok(_) => ProcessResult::Progress,
                        Err(_) => ProcessResult::BadSignature,
                    };
                    if self.result == ProcessResult::Progress {
                        let block_details = BlockDetails::new(
                            Epoch::Epoch0,
                            false, /* unused */
                            false, /* unused */
                            false, /* unused */
                        );
                        // Does this block have sufficient work? (Malformed)
                        self.result = if self.constants.work.difficulty_block(block)
                            >= self
                                .constants
                                .work
                                .threshold2(block.work_version(), &block_details)
                        {
                            ProcessResult::Progress
                        } else {
                            ProcessResult::InsufficientWork
                        };
                        if self.result == ProcessResult::Progress {
                            debug_assert!(validate_message(
                                &account.into(),
                                hash.as_bytes(),
                                block.block_signature()
                            )
                            .is_ok());
                            block.set_sideband(BlockSideband::new(
                                account,
                                BlockHash::zero(),
                                info.balance,
                                info.block_count + 1,
                                seconds_since_epoch(),
                                block_details,
                                Epoch::Epoch0, /* unused */
                            ));
                            self.ledger.store.block().put(self.txn, &hash, block);
                            let balance = self.ledger.balance(self.txn.txn(), &block.previous());
                            self.ledger.cache.rep_weights.representation_add_dual(
                                block.representative(),
                                balance,
                                info.representative,
                                Amount::zero().wrapping_sub(balance),
                            );
                            let new_info = AccountInfo {
                                head: hash,
                                representative: block.representative(),
                                open_block: info.open_block,
                                balance: info.balance,
                                modified: seconds_since_epoch(),
                                block_count: info.block_count + 1,
                                epoch: Epoch::Epoch0,
                            };
                            self.ledger
                                .update_account(self.txn, &account, &info, &new_info);
                            self.ledger
                                .store
                                .frontier()
                                .del(self.txn, &block.previous());
                            self.ledger.store.frontier().put(self.txn, &hash, &account);
                            self.observer.block_added(BlockSubType::Change);
                        }
                    }
                }
            }
        }
    }

    fn state_block(&mut self, block: &mut StateBlock) {
        self.result = ProcessResult::Progress;
        let mut is_epoch_block = false;
        if self.ledger.is_epoch_link(&block.link()) {
            // This function also modifies the result variable if epoch is mal-formed
            is_epoch_block = self.validate_epoch_block(block);
        }

        if self.result == ProcessResult::Progress {
            if is_epoch_block {
                self.epoch_block_impl(block);
            } else {
                self.result = match StateBlockProcessor::new(self.ledger, self.txn, block).process()
                {
                    Ok(res) => res,
                    Err(res) => res,
                }
            }
        }
    }
}

// Processes state blocks that don't have an epoch link
struct StateBlockProcessor<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
    block: &'a mut StateBlock,
    old_account_info: Option<AccountInfo>,
}

impl<'a> StateBlockProcessor<'a> {
    fn new(
        ledger: &'a Ledger,
        txn: &'a mut dyn WriteTransaction,
        block: &'a mut StateBlock,
    ) -> Self {
        Self {
            ledger,
            txn,
            block,
            old_account_info: None,
        }
    }

    /// Have we seen this block before? (Unambiguous)
    fn ensure_new_block(&self) -> Result<(), ProcessResult> {
        if self
            .ledger
            .block_or_pruned_exists_txn(self.txn.txn(), &self.block.hash())
        {
            Err(ProcessResult::Old)
        } else {
            Ok(())
        }
    }

    /// Is this block signed correctly (Unambiguous)
    fn ensure_valid_block_signature(&self) -> Result<(), ProcessResult> {
        validate_block_signature(self.block).map_err(|_| ProcessResult::BadSignature)
    }

    /// Is this for the burn account? (Unambiguous)
    fn ensure_not_for_burn_account(&self) -> Result<(), ProcessResult> {
        if self.block.account().is_zero() {
            Err(ProcessResult::OpenedBurnAccount)
        } else {
            Ok(())
        }
    }

    /// Does the previous block exist in the ledger? (Unambigious)
    fn ensure_previous_block_exists(&self) -> Result<(), ProcessResult> {
        if self.old_account_info.is_some() {
            if self
                .ledger
                .store
                .block()
                .exists(self.txn.txn(), &self.block.previous())
            {
                Ok(())
            } else {
                Err(ProcessResult::GapPrevious)
            }
        } else {
            // Does the first block in an account yield 0 for previous() ? (Unambigious)
            if self.block.previous().is_zero() {
                Ok(())
            } else {
                Err(ProcessResult::GapPrevious)
            }
        }
    }

    fn ensure_no_double_account_open(&self) -> Result<(), ProcessResult> {
        if self.old_account_info.is_none() || !self.block.previous().is_zero() {
            Ok(())
        } else {
            Err(ProcessResult::Fork)
        }
    }

    fn ensure_new_account_has_link(&self) -> Result<(), ProcessResult> {
        if self.old_account_info.is_some() || !self.block.link().is_zero() {
            Ok(())
        } else {
            Err(ProcessResult::GapSource)
        }
    }

    fn ensure_previous_block_is_account_head(&self) -> Result<(), ProcessResult> {
        if let Some(info) = &self.old_account_info {
            // Is the previous block the account's head block? (Ambigious)
            if self.block.previous() != info.head {
                return Err(ProcessResult::Fork);
            }
        }

        Ok(())
    }

    fn process(&mut self) -> Result<ProcessResult, ProcessResult> {
        self.old_account_info = self
            .ledger
            .get_account_info(self.txn.txn(), &self.block.account());

        self.ensure_new_block()?;
        self.ensure_valid_block_signature()?;
        self.ensure_not_for_burn_account()?;
        self.ensure_no_double_account_open()?;
        self.ensure_previous_block_exists()?;
        self.ensure_previous_block_is_account_head()?;
        self.ensure_new_account_has_link()?;

        let hash = self.block.hash();
        let mut epoch: Epoch;
        let mut source_epoch: Epoch;
        let amount: Amount;
        let is_send: bool;
        let is_receive: bool;

        match &self.old_account_info {
            Some(info) => {
                epoch = info.epoch;
                source_epoch = Epoch::Epoch0;
                is_send = self.block.balance() < info.balance;
                amount = if is_send {
                    info.balance - self.block.balance()
                } else {
                    self.block.balance() - info.balance
                };
                is_receive = !is_send && !self.block.link().is_zero();
            }
            None => {
                epoch = Epoch::Epoch0;
                source_epoch = Epoch::Epoch0;
                amount = self.block.balance();
                is_send = false;
                is_receive = true;
            }
        }

        if !is_send {
            if !self.block.link().is_zero() {
                // Have we seen the source block already? (Harmless)
                if !self
                    .ledger
                    .block_or_pruned_exists_txn(self.txn.txn(), &self.block.link().into())
                {
                    return Err(ProcessResult::GapSource);
                }

                let key = PendingKey::new(self.block.account(), self.block.link().into());
                // Has this source already been received (Malformed)
                match self.ledger.store.pending().get(self.txn.txn(), &key) {
                    Some(pending) => {
                        if amount != pending.amount {
                            return Err(ProcessResult::BalanceMismatch);
                        }
                        source_epoch = pending.epoch;
                        epoch = std::cmp::max(epoch, source_epoch);
                    }
                    None => {
                        return Err(ProcessResult::Unreceivable);
                    }
                };
            } else {
                // If there's no link, the balance must remain the same, only the representative can change
                if !amount.is_zero() {
                    return Err(ProcessResult::BalanceMismatch);
                }
            }
        }

        let block_details = BlockDetails::new(epoch, is_send, is_receive, false);
        // Does this block have sufficient work? (Malformed)
        if self.ledger.constants.work.difficulty_block(self.block)
            < self
                .ledger
                .constants
                .work
                .threshold2(self.block.work_version(), &block_details)
        {
            return Err(ProcessResult::InsufficientWork);
        }

        self.ledger.observer.state_block_added();
        self.block.set_sideband(BlockSideband::new(
            self.block.account(), /* unused */
            BlockHash::zero(),
            Amount::zero(), /* unused */
            self.old_account_info
                .as_ref()
                .map(|i| i.block_count)
                .unwrap_or_default()
                + 1,
            seconds_since_epoch(),
            block_details,
            source_epoch,
        ));
        self.ledger.store.block().put(self.txn, &hash, self.block);

        if let Some(acc_info) = &self.old_account_info {
            // Move existing representation & add in amount delta
            self.ledger.cache.rep_weights.representation_add_dual(
                acc_info.representative,
                Amount::zero().wrapping_sub(acc_info.balance),
                self.block.representative(),
                self.block.balance(),
            );
        } else {
            // Add in amount delta only
            self.ledger
                .cache
                .rep_weights
                .representation_add(self.block.representative(), self.block.balance());
        }

        if is_send {
            let key = PendingKey::new(self.block.link().into(), hash);
            let info = PendingInfo::new(self.block.account(), amount, epoch);
            self.ledger.store.pending().put(self.txn, &key, &info);
        } else if !self.block.link().is_zero() {
            self.ledger.store.pending().del(
                self.txn,
                &PendingKey::new(self.block.account(), self.block.link().into()),
            );
        }

        let new_info = AccountInfo {
            head: hash,
            representative: self.block.representative(),
            open_block: if let Some(acc_info) = &self.old_account_info {
                acc_info.open_block
            } else {
                hash
            },
            balance: self.block.balance(),
            modified: seconds_since_epoch(),
            block_count: self
                .old_account_info
                .as_ref()
                .map(|a| a.block_count)
                .unwrap_or_default()
                + 1,
            epoch,
        };

        self.ledger.update_account(
            self.txn,
            &self.block.account(),
            &self.old_account_info.clone().unwrap_or_default(),
            &new_info,
        );

        if let Some(acc_info) = &self.old_account_info {
            if self
                .ledger
                .store
                .frontier()
                .get(self.txn.txn(), &acc_info.head)
                .is_some()
            {
                self.ledger.store.frontier().del(self.txn, &acc_info.head);
            }
        }

        Ok(ProcessResult::Progress)
    }
}
