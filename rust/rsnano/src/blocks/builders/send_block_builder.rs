use crate::{
    core::{BlockHash, KeyPair},
    Account, Amount, Block, BlockDetails, BlockSideband, Epoch, SendBlock,
};

#[derive(Default)]
pub struct SendBlockBuilder {
    previous: Option<BlockHash>,
}

impl SendBlockBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn previous(mut self, hash: BlockHash) -> Self {
        self.previous = Some(hash);
        self
    }

    pub fn build(self) -> anyhow::Result<SendBlock> {
        let key_pair = KeyPair::new();
        let previous = self.previous.unwrap_or(BlockHash::from(1));
        let destination = Account::from(2);
        let balance = Amount::new(3);
        let mut block = SendBlock::new(
            &previous,
            &destination,
            &balance,
            &key_pair.private_key(),
            &key_pair.public_key(),
            4,
        )?;
        let details = BlockDetails::new(Epoch::Epoch0, true, false, false);
        block.set_sideband(BlockSideband::new(
            Account::from(4),
            BlockHash::new(),
            balance,
            5,
            8,
            details,
            Epoch::Epoch0,
        ));
        Ok(block)
    }
}
