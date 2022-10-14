use crate::{Amount, Block, BlockDetails, BlockHash, BlockSideband, Epoch, KeyPair, ReceiveBlock};

pub struct ReceiveBlockBuilder {}

impl ReceiveBlockBuilder {
    pub fn new() -> Self {
        Self {}
    }

    pub fn build(self) -> anyhow::Result<ReceiveBlock> {
        let key_pair = KeyPair::new();

        let mut block = ReceiveBlock::new(
            BlockHash::from(1),
            BlockHash::from(2),
            &key_pair.private_key(),
            &key_pair.public_key(),
            4,
        )?;

        let details = BlockDetails {
            epoch: crate::Epoch::Epoch0,
            is_send: false,
            is_receive: true,
            is_epoch: false,
        };
        block.set_sideband(BlockSideband::new(
            *block.account(),
            *BlockHash::zero(),
            Amount::new(5),
            1,
            2,
            details,
            Epoch::Epoch0,
        ));

        Ok(block)
    }
}

#[cfg(test)]
mod tests {
    use crate::BlockBuilder;

    #[test]
    fn receive_block() {
        let block = BlockBuilder::receive().build();
        assert!(false)
    }
}
