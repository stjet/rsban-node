use crate::core::{
    Account, Amount, Block, BlockDetails, BlockHash, BlockSideband, ChangeBlock, Epoch, KeyPair,
};

pub struct ChangeBlockBuilder {
    account: Option<Account>,
    representative: Option<Account>,
    previous: Option<BlockHash>,
    keypair: Option<KeyPair>,
    work: Option<u64>,
    build_sideband: bool,
}

impl ChangeBlockBuilder {
    pub fn new() -> Self {
        Self {
            account: None,
            representative: None,
            previous: None,
            keypair: None,
            work: None,
            build_sideband: true,
        }
    }

    pub fn previous(mut self, previous: BlockHash) -> Self {
        self.previous = Some(previous);
        self
    }

    pub fn account(mut self, account: Account) -> Self {
        self.account = Some(account);
        self
    }

    pub fn representative(mut self, representative: Account) -> Self {
        self.representative = Some(representative);
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

    pub fn build(self) -> anyhow::Result<ChangeBlock> {
        let previous = self.previous.unwrap_or(BlockHash::from(1));
        let key_pair = self.keypair.unwrap_or_default();
        let representative = self.representative.unwrap_or(Account::from(2));
        let work = self.work.unwrap_or(4);

        let mut block = ChangeBlock::new(
            previous,
            representative,
            &key_pair.private_key(),
            &key_pair.public_key(),
            work,
        )?;

        if self.build_sideband {
            let details = BlockDetails {
                epoch: Epoch::Epoch0,
                is_send: false,
                is_receive: true,
                is_epoch: false,
            };
            block.set_sideband(BlockSideband::new(
                block.account(),
                BlockHash::zero(),
                Amount::new(5),
                1,
                2,
                details,
                Epoch::Epoch0,
            ));
        }

        Ok(block)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{BlockBuilder, Signature};

    #[test]
    fn create_open_block() {
        let block = BlockBuilder::open().build().unwrap();
        assert_eq!(block.hashables.source, BlockHash::from(1));
        assert_eq!(block.hashables.representative, Account::from(2));
        assert_ne!(block.hashables.account, Account::zero());
        assert_eq!(block.work, 4);
        assert_ne!(*block.block_signature(), Signature::new());

        let sideband = block.sideband().unwrap();
        assert_eq!(sideband.account, block.account());
        assert!(sideband.successor.is_zero());
        assert_eq!(sideband.balance, Amount::new(5));
        assert_eq!(sideband.height, 1);
        assert_eq!(sideband.timestamp, 2);
        assert_eq!(sideband.source_epoch, Epoch::Epoch0);
    }
}
