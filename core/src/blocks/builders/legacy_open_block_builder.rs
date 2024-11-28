use crate::{
    work::{WorkPool, STUB_WORK_POOL},
    Account, Amount, Block, BlockBase, BlockDetails, BlockHash, BlockSideband, Epoch, OpenBlock,
    PrivateKey, PublicKey,
};

pub struct LegacyOpenBlockBuilder {
    account: Option<Account>,
    representative: Option<PublicKey>,
    source: Option<BlockHash>,
    prv_key: Option<PrivateKey>,
    work: Option<u64>,
    build_sideband: bool,
    height: Option<u64>,
}

impl LegacyOpenBlockBuilder {
    pub fn new() -> Self {
        Self {
            account: None,
            representative: None,
            source: None,
            prv_key: None,
            work: None,
            build_sideband: false,
            height: None,
        }
    }

    pub fn source(mut self, source: BlockHash) -> Self {
        self.source = Some(source);
        self
    }

    pub fn account(mut self, account: Account) -> Self {
        self.account = Some(account);
        self
    }

    pub fn representative(mut self, representative: PublicKey) -> Self {
        self.representative = Some(representative);
        self
    }

    pub fn sign(mut self, prv_key: &PrivateKey) -> Self {
        self.prv_key = Some(prv_key.clone());
        self
    }

    pub fn work(mut self, work: u64) -> Self {
        self.work = Some(work);
        self
    }

    pub fn with_sideband(mut self) -> Self {
        self.build_sideband = true;
        self
    }

    pub fn height(mut self, height: u64) -> Self {
        self.height = Some(height);
        self
    }

    pub fn build(self) -> Block {
        let source = self.source.unwrap_or(BlockHash::from(1));
        let prv_key = self.prv_key.unwrap_or_default();
        let account = self.account.unwrap_or_else(|| prv_key.account());
        let representative = self.representative.unwrap_or(PublicKey::from(2));
        let work = self
            .work
            .unwrap_or_else(|| STUB_WORK_POOL.generate_dev2(account.into()).unwrap());

        let mut block = OpenBlock::new(source, representative, account, &prv_key, work);

        let details = BlockDetails {
            epoch: Epoch::Epoch0,
            is_send: false,
            is_receive: true,
            is_epoch: false,
        };

        if self.build_sideband || self.height.is_some() {
            let height = self.height.unwrap_or(1);
            block.set_sideband(BlockSideband::new(
                block.account(),
                BlockHash::zero(),
                Amount::raw(5),
                height,
                2,
                details,
                Epoch::Epoch0,
            ));
        }

        Block::LegacyOpen(block)
    }
}

impl Default for LegacyOpenBlockBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{work::WORK_THRESHOLDS_STUB, BlockBuilder, Signature};

    #[test]
    fn create_open_block() {
        let block = BlockBuilder::legacy_open().with_sideband().build();
        let Block::LegacyOpen(open) = &block else {
            panic!("not an open block")
        };
        assert_eq!(open.hashables.source, BlockHash::from(1));
        assert_eq!(open.hashables.representative, PublicKey::from(2));
        assert_ne!(open.account(), Account::zero());
        assert_eq!(WORK_THRESHOLDS_STUB.validate_entry_block(&block), true);
        assert_ne!(*open.block_signature(), Signature::new());

        let sideband = open.sideband().unwrap();
        assert_eq!(sideband.account, open.account());
        assert!(sideband.successor.is_zero());
        assert_eq!(sideband.balance, Amount::raw(5));
        assert_eq!(sideband.height, 1);
        assert_eq!(sideband.timestamp, 2);
        assert_eq!(sideband.source_epoch, Epoch::Epoch0);
    }
}
