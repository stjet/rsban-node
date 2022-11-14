use crate::{
    core::{
        Account, Amount, Block, BlockDetails, BlockHash, BlockSideband, Epoch, KeyPair, SendBlock,
    },
    work::DEV_WORK_POOL,
};

pub struct SendBlockBuilder {
    account: Option<Account>,
    previous: Option<BlockHash>,
    destination: Option<Account>,
    balance: Option<Amount>,
    work: Option<u64>,
    keypair: Option<KeyPair>,
    build_sideband: bool,
}

impl SendBlockBuilder {
    pub fn new() -> Self {
        Self {
            account: None,
            previous: None,
            destination: None,
            balance: None,
            work: None,
            keypair: None,
            build_sideband: true,
        }
    }

    pub fn account(mut self, account: Account) -> Self {
        self.account = Some(account);
        self
    }

    pub fn previous(mut self, hash: BlockHash) -> Self {
        self.previous = Some(hash);
        self
    }

    pub fn destination(mut self, destination: Account) -> Self {
        self.destination = Some(destination);
        self
    }

    pub fn balance(mut self, balance: Amount) -> Self {
        self.balance = Some(balance);
        self
    }

    pub fn sign(mut self, keypair: KeyPair) -> Self {
        self.keypair = Some(keypair);
        self
    }

    pub fn work(mut self, work: u64) -> Self {
        self.work = Some(work);
        self
    }

    pub fn without_sideband(mut self) -> Self {
        self.build_sideband = false;
        self
    }

    pub fn build(self) -> anyhow::Result<SendBlock> {
        let key_pair = self.keypair.unwrap_or_default();
        let previous = self.previous.unwrap_or(BlockHash::from(1));
        let destination = self.destination.unwrap_or(Account::from(2));
        let balance = self.balance.unwrap_or(Amount::new(3));
        let work = self
            .work
            .unwrap_or_else(|| DEV_WORK_POOL.generate_dev2(previous.into()).unwrap());
        let mut block = SendBlock::new(
            &previous,
            &destination,
            &balance,
            &key_pair.private_key(),
            &key_pair.public_key(),
            work,
        )?;

        if self.build_sideband {
            let details = BlockDetails::new(Epoch::Epoch0, true, false, false);
            block.set_sideband(BlockSideband::new(
                self.account.unwrap_or(Account::from(4)),
                BlockHash::zero(),
                balance,
                5,
                8,
                details,
                Epoch::Epoch0,
            ));
        }
        Ok(block)
    }
}
