use crate::{
    blocks::state_block::EpochBlockArgs,
    dev_epoch1_signer, epoch_v1_link,
    work::{WorkPool, WorkPoolImpl},
    Account, Amount, Block, BlockHash, Epoch, Link, PendingInfo, PendingKey, PrivateKey, PublicKey,
    Root, StateBlockArgs, DEV_GENESIS_BLOCK, DEV_GENESIS_KEY,
};
use std::collections::HashMap;

pub struct UnsavedBlockLatticeBuilder {
    accounts: HashMap<Account, Frontier>,
    work_pool: WorkPoolImpl,
    pending_receives: HashMap<PendingKey, PendingInfo>,
}

#[derive(Clone)]
struct Frontier {
    hash: BlockHash,
    representative: PublicKey,
    balance: Amount,
}

impl UnsavedBlockLatticeBuilder {
    pub fn new() -> Self {
        let mut accounts = HashMap::new();
        accounts.insert(
            DEV_GENESIS_KEY.account(),
            Frontier {
                hash: DEV_GENESIS_BLOCK.hash(),
                representative: DEV_GENESIS_KEY.public_key(),
                balance: Amount::MAX,
            },
        );
        let work_pool = WorkPoolImpl::new_dev();
        Self {
            accounts,
            work_pool,
            pending_receives: Default::default(),
        }
    }

    pub fn genesis(&mut self) -> UnsavedAccountChainBuilder {
        self.account(&DEV_GENESIS_KEY)
    }

    pub fn account<'a>(&'a mut self, key: &'a PrivateKey) -> UnsavedAccountChainBuilder<'a> {
        UnsavedAccountChainBuilder { lattice: self, key }
    }

    pub fn epoch_open(&mut self, account: impl Into<Account>) -> Block {
        let account = account.into();
        assert!(!self.accounts.contains_key(&account));
        assert!(self
            .pending_receives
            .keys()
            .any(|k| k.receiving_account == account));

        let receive: Block = EpochBlockArgs {
            epoch_signer: dev_epoch1_signer(),
            account,
            previous: BlockHash::zero(),
            representative: PublicKey::zero(),
            balance: Amount::zero(),
            link: epoch_v1_link(),
            work: self.work_pool.generate_dev2(account.into()).unwrap(),
        }
        .into();

        self.accounts.insert(
            account,
            Frontier {
                hash: receive.hash(),
                representative: PublicKey::zero(),
                balance: Amount::zero(),
            },
        );

        receive
    }
}

pub struct UnsavedAccountChainBuilder<'a> {
    lattice: &'a mut UnsavedBlockLatticeBuilder,
    key: &'a PrivateKey,
}

impl<'a> UnsavedAccountChainBuilder<'a> {
    pub fn send_all_except(
        &mut self,
        destination: impl Into<Account>,
        keep: impl Into<Amount>,
    ) -> Block {
        let frontier = self.get_frontier();
        self.send(destination, frontier.balance - keep.into())
    }

    pub fn send(&mut self, destination: impl Into<Account>, amount: impl Into<Amount>) -> Block {
        let destination = destination.into();
        let frontier = self.get_frontier();
        let amount = amount.into();
        let new_balance = frontier.balance - amount;

        let send: Block = StateBlockArgs {
            key: self.key,
            previous: frontier.hash,
            representative: frontier.representative,
            balance: new_balance,
            link: destination.into(),
            work: self
                .lattice
                .work_pool
                .generate_dev2(frontier.hash.into())
                .unwrap(),
        }
        .into();

        self.set_new_frontier(Frontier {
            hash: send.hash(),
            representative: frontier.representative,
            balance: new_balance,
        });

        self.lattice.pending_receives.insert(
            PendingKey::new(destination, send.hash()),
            PendingInfo {
                source: self.key.account(),
                amount,
                epoch: Epoch::Epoch0,
            },
        );

        send
    }

    pub fn receive(&mut self, corresponding_send: &Block) -> Block {
        assert_eq!(corresponding_send.destination_or_link(), self.key.account());
        let amount = self
            .lattice
            .pending_receives
            .remove(&PendingKey::new(
                self.key.account(),
                corresponding_send.hash(),
            ))
            .expect("no pending receive found")
            .amount;

        let frontier = self.get_frontier_or_empty();

        let root: Root = if frontier.hash.is_zero() {
            self.key.account().into()
        } else {
            frontier.hash.into()
        };

        let new_balance = frontier.balance + amount;

        let receive: Block = StateBlockArgs {
            key: self.key,
            previous: frontier.hash,
            representative: frontier.representative,
            balance: new_balance,
            link: corresponding_send.hash().into(),
            work: self.lattice.work_pool.generate_dev2(root).unwrap(),
        }
        .into();

        self.set_new_frontier(Frontier {
            hash: receive.hash(),
            representative: frontier.representative,
            balance: new_balance,
        });

        receive
    }

    pub fn change(&mut self, new_representative: impl Into<PublicKey>) -> Block {
        let frontier = self.get_frontier();
        let new_representative = new_representative.into();
        let change: Block = StateBlockArgs {
            key: self.key,
            previous: frontier.hash,
            representative: new_representative,
            balance: frontier.balance,
            link: Link::zero(),
            work: self
                .lattice
                .work_pool
                .generate_dev2(frontier.hash.into())
                .unwrap(),
        }
        .into();

        self.set_new_frontier(Frontier {
            hash: change.hash(),
            representative: new_representative,
            balance: frontier.balance,
        });

        change
    }

    fn set_new_frontier(&mut self, new_frontier: Frontier) {
        self.lattice
            .accounts
            .insert(self.key.account(), new_frontier);
    }

    fn get_frontier(&self) -> Frontier {
        self.lattice
            .accounts
            .get(&self.key.account())
            .expect("Cannot send/change from unopenend account!")
            .clone()
    }

    fn get_frontier_or_empty(&self) -> Frontier {
        self.lattice
            .accounts
            .get(&self.key.account())
            .cloned()
            .unwrap_or_else(|| Frontier {
                hash: BlockHash::zero(),
                representative: self.key.public_key(),
                balance: Amount::zero(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{work::WorkThresholds, BlockDetails};

    #[test]
    fn state_send() {
        let mut lattice = UnsavedBlockLatticeBuilder::new();
        let key1 = PrivateKey::from(42);

        let send = lattice.genesis().send(&key1, 1);

        let expected: Block = StateBlockArgs {
            key: &DEV_GENESIS_KEY,
            previous: DEV_GENESIS_BLOCK.hash(),
            representative: DEV_GENESIS_KEY.public_key(),
            balance: Amount::MAX - Amount::raw(1),
            link: key1.account().into(),
            work: send.work(),
        }
        .into();
        assert_eq!(send, expected);
        assert!(WorkThresholds::publish_dev().is_valid_pow(
            &send,
            &BlockDetails::new(crate::Epoch::Epoch2, true, false, false)
        ))
    }

    #[test]
    fn send_twice() {
        let mut lattice = UnsavedBlockLatticeBuilder::new();
        let key1 = PrivateKey::from(42);

        let send1 = lattice.genesis().send(&key1, 1);
        let send2 = lattice.genesis().send(&key1, 2);

        let expected: Block = StateBlockArgs {
            key: &DEV_GENESIS_KEY,
            previous: send1.hash(),
            representative: DEV_GENESIS_KEY.public_key(),
            balance: Amount::MAX - Amount::raw(3),
            link: key1.account().into(),
            work: send2.work(),
        }
        .into();
        assert_eq!(send2, expected);
    }

    #[test]
    fn state_open() {
        let mut lattice = UnsavedBlockLatticeBuilder::new();
        let key1 = PrivateKey::from(42);
        let send = lattice.genesis().send(&key1, 1);

        let open = lattice.account(&key1).receive(&send);

        let expected: Block = StateBlockArgs {
            key: &key1,
            previous: BlockHash::zero(),
            representative: key1.public_key(),
            balance: Amount::raw(1),
            link: send.hash().into(),
            work: open.work(),
        }
        .into();
        assert_eq!(open, expected);
        assert!(WorkThresholds::publish_dev().is_valid_pow(
            &send,
            &BlockDetails::new(crate::Epoch::Epoch2, false, true, false)
        ))
    }

    #[test]
    fn state_receive() {
        let mut lattice = UnsavedBlockLatticeBuilder::new();
        let key1 = PrivateKey::from(42);
        let send1 = lattice.genesis().send(&key1, 1);
        let send2 = lattice.genesis().send(&key1, 2);
        let open = lattice.account(&key1).receive(&send1);

        let receive = lattice.account(&key1).receive(&send2);

        let expected: Block = StateBlockArgs {
            key: &key1,
            previous: open.hash(),
            representative: key1.public_key(),
            balance: Amount::raw(3),
            link: send2.hash().into(),
            work: receive.work(),
        }
        .into();
        assert_eq!(receive, expected);
    }
}
