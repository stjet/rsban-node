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
    pending_receive: Option<PendingInfo>,
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
            pending_receive: None,
        }
    }

    fn account_exists(&self) -> bool {
        self.old_account_info.is_some()
    }

    fn is_new_account(&self) -> bool {
        self.old_account_info.is_none()
    }

    fn is_send(&self) -> bool {
        match &self.old_account_info {
            Some(info) => self.block.balance() < info.balance,
            None => false,
        }
    }

    fn is_receive(&self) -> bool {
        match &self.old_account_info {
            Some(info) => self.block.balance() >= info.balance && !self.block.link().is_zero(),
            None => true,
        }
    }

    fn amount(&self) -> Amount {
        match &self.old_account_info {
            Some(info) => {
                if self.is_send() {
                    info.balance - self.block.balance()
                } else {
                    self.block.balance() - info.balance
                }
            }
            None => self.block.balance(),
        }
    }

    fn epoch(&self) -> Epoch {
        let epoch = self
            .old_account_info
            .as_ref()
            .map(|i| i.epoch)
            .unwrap_or(Epoch::Epoch0);

        std::cmp::max(epoch, self.source_epoch())
    }

    fn source_epoch(&self) -> Epoch {
        self.pending_receive
            .as_ref()
            .map(|p| p.epoch)
            .unwrap_or(Epoch::Epoch0)
    }

    /// Have we seen this block before? (Unambiguous)
    fn ensure_block_does_not_exist_yet(&self) -> Result<(), ProcessResult> {
        if self
            .ledger
            .block_or_pruned_exists_txn(self.txn.txn(), &self.block.hash())
        {
            return Err(ProcessResult::Old);
        }
        Ok(())
    }

    /// Is this block signed correctly (Unambiguous)
    fn ensure_valid_block_signature(&self) -> Result<(), ProcessResult> {
        validate_block_signature(self.block).map_err(|_| ProcessResult::BadSignature)
    }

    /// Is this for the burn account? (Unambiguous)
    fn ensure_block_is_not_for_burn_account(&self) -> Result<(), ProcessResult> {
        if self.block.account().is_zero() {
            Err(ProcessResult::OpenedBurnAccount)
        } else {
            Ok(())
        }
    }

    /// Does the previous block exist in the ledger? (Unambigious)
    fn ensure_previous_block_exists(&self) -> Result<(), ProcessResult> {
        if self.account_exists()
            && !self
                .ledger
                .store
                .block()
                .exists(self.txn.txn(), &self.block.previous())
        {
            return Err(ProcessResult::GapPrevious);
        }

        if self.is_new_account() && !self.block.previous().is_zero() {
            return Err(ProcessResult::GapPrevious);
        }

        Ok(())
    }

    fn ensure_no_double_account_open(&self) -> Result<(), ProcessResult> {
        if self.account_exists() && self.block.previous().is_zero() {
            Err(ProcessResult::Fork)
        } else {
            Ok(())
        }
    }

    fn ensure_new_account_has_link(&self) -> Result<(), ProcessResult> {
        if self.is_new_account() && self.block.link().is_zero() {
            Err(ProcessResult::GapSource)
        } else {
            Ok(())
        }
    }

    /// Is the previous block the account's head block? (Ambigious)
    fn ensure_previous_block_is_account_head(&self) -> Result<(), ProcessResult> {
        if let Some(info) = &self.old_account_info {
            if self.block.previous() != info.head {
                return Err(ProcessResult::Fork);
            }
        }

        Ok(())
    }

    fn ensure_link_block_exists(&self) -> Result<(), ProcessResult> {
        if !self
            .ledger
            .block_or_pruned_exists_txn(self.txn.txn(), &self.block.link().into())
        {
            Err(ProcessResult::GapSource)
        } else {
            Ok(())
        }
    }

    /// If there's no link, the balance must remain the same, only the representative can change
    fn ensure_no_receive_balance_change_without_link(&self) -> Result<(), ProcessResult> {
        if !self.is_send() && self.block.link().is_zero() {
            if !self.amount().is_zero() {
                return Err(ProcessResult::BalanceMismatch);
            }
        }

        Ok(())
    }

    fn ensure_receive_block_links_to_existing_block(&self) -> Result<(), ProcessResult> {
        if self.is_receive() {
            self.ensure_link_block_exists()?;
        }
        Ok(())
    }

    fn ensure_receive_block_receives_pending_amount(&self) -> Result<(), ProcessResult> {
        if self.is_receive() {
            match &self.pending_receive {
                Some(pending) => {
                    if self.amount() != pending.amount {
                        return Err(ProcessResult::BalanceMismatch);
                    }
                }
                None => {
                    return Err(ProcessResult::Unreceivable);
                }
            };
        }

        Ok(())
    }

    fn ensure_sufficient_work(&self) -> Result<(), ProcessResult> {
        if self.ledger.constants.work.difficulty_block(self.block)
            < self
                .ledger
                .constants
                .work
                .threshold2(self.block.work_version(), &self.block_details())
        {
            Err(ProcessResult::InsufficientWork)
        } else {
            Ok(())
        }
    }

    fn ensure_valid_state_block(&self) -> Result<(), ProcessResult> {
        self.ensure_block_does_not_exist_yet()?;
        self.ensure_valid_block_signature()?;
        self.ensure_block_is_not_for_burn_account()?;
        self.ensure_no_double_account_open()?;
        self.ensure_previous_block_exists()?;
        self.ensure_previous_block_is_account_head()?;
        self.ensure_new_account_has_link()?;
        self.ensure_no_receive_balance_change_without_link()?;
        self.ensure_receive_block_links_to_existing_block()?;
        self.ensure_receive_block_receives_pending_amount()?;
        self.ensure_sufficient_work()
    }

    fn process(&mut self) -> Result<ProcessResult, ProcessResult> {
        self.initialize();
        self.ensure_valid_state_block()?;

        self.ledger.observer.state_block_added();

        self.block.set_sideband(self.create_sideband());

        self.ledger
            .store
            .block()
            .put(self.txn, &self.block.hash(), self.block);

        self.update_representative_cache();

        if self.is_send() {
            self.add_pending_receive();
        } else if self.is_receive() {
            self.delete_pending_receive();
        }

        self.update_account_info();
        self.delete_frontier();
        Ok(ProcessResult::Progress)
    }

    fn delete_frontier(&mut self) {
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
    }

    fn update_account_info(&mut self) {
        let new_account_info = self.create_account_info();
        self.ledger.update_account(
            self.txn,
            &self.block.account(),
            &self.old_account_info.clone().unwrap_or_default(),
            &new_account_info,
        );
    }

    fn add_pending_receive(&mut self) {
        let key = PendingKey::for_send_block(self.block);
        let info = PendingInfo::new(self.block.account(), self.amount(), self.epoch());
        self.ledger.store.pending().put(self.txn, &key, &info);
    }

    fn delete_pending_receive(&mut self) {
        self.ledger
            .store
            .pending()
            .del(self.txn, &PendingKey::for_receive_block(self.block));
    }

    fn update_representative_cache(&mut self) {
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
    }

    fn create_account_info(&self) -> AccountInfo {
        AccountInfo {
            head: self.block.hash(),
            representative: self.block.representative(),
            open_block: if let Some(acc_info) = &self.old_account_info {
                acc_info.open_block
            } else {
                self.block.hash()
            },
            balance: self.block.balance(),
            modified: seconds_since_epoch(),
            block_count: self
                .old_account_info
                .as_ref()
                .map(|a| a.block_count)
                .unwrap_or_default()
                + 1,
            epoch: self.epoch(),
        }
    }

    fn create_sideband(&self) -> BlockSideband {
        BlockSideband::new(
            self.block.account(), /* unused */
            BlockHash::zero(),
            Amount::zero(), /* unused */
            self.old_account_info
                .as_ref()
                .map(|i| i.block_count)
                .unwrap_or_default()
                + 1,
            seconds_since_epoch(),
            self.block_details(),
            self.source_epoch(),
        )
    }

    fn block_details(&self) -> BlockDetails {
        BlockDetails::new(self.epoch(), self.is_send(), self.is_receive(), false)
    }

    fn initialize(&mut self) {
        self.old_account_info = self.get_old_account_info();

        if self.is_receive() {
            self.pending_receive = self
                .ledger
                .store
                .pending()
                .get(self.txn.txn(), &PendingKey::for_receive_block(self.block));
        }
    }

    fn get_old_account_info(&mut self) -> Option<AccountInfo> {
        self.ledger
            .get_account_info(self.txn.txn(), &self.block.account())
    }
}
