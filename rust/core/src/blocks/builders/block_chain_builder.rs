use crate::{
    Account, Amount, BlockBuilder, BlockDetails, BlockEnum, BlockHash, BlockSideband, Epoch,
    LegacySendBlockBuilder,
};

pub struct BlockChainBuilder {
    account: Account,
    blocks: Vec<BlockEnum>,
}

impl BlockChainBuilder {
    pub fn new() -> Self {
        Self::for_account(42)
    }

    pub fn for_account<T: Into<Account>>(account: T) -> Self {
        Self {
            account: account.into(),
            blocks: Vec::new(),
        }
    }

    pub fn from_send_block(block: &BlockEnum) -> Self {
        let BlockEnum::LegacySend(send_block) = block else {
            panic!("not a send block!")
        };

        Self::for_account(*send_block.mandatory_destination()).legacy_open_from(block)
    }

    pub fn height(&self) -> u64 {
        self.blocks.len() as u64
    }

    pub fn open(&self) -> BlockHash {
        self.blocks[0].hash()
    }

    pub fn frontier(&self) -> BlockHash {
        self.blocks.last().unwrap().hash()
    }

    pub fn account(&self) -> Account {
        self.account
    }

    pub fn blocks(&self) -> &[BlockEnum] {
        &self.blocks
    }

    pub fn latest_block(&self) -> &BlockEnum {
        self.blocks.last().unwrap()
    }

    fn add_block(&mut self, mut block: BlockEnum) -> &BlockEnum {
        block.set_sideband(BlockSideband {
            height: self.height() + 1,
            timestamp: 1,
            successor: BlockHash::zero(),
            account: self.account,
            balance: Amount::zero(),
            details: BlockDetails::new(Epoch::Unspecified, false, false, false),
            source_epoch: Epoch::Unspecified,
        });

        if self.blocks.len() > 0 {
            let previous = self.blocks.last_mut().unwrap();
            let mut sideband = previous.sideband().unwrap().clone();
            sideband.successor = block.hash();
            previous.set_sideband(sideband);
        }

        self.blocks.push(block);
        self.blocks.last().unwrap()
    }

    pub fn legacy_open(mut self) -> Self {
        let block_builder = BlockBuilder::legacy_open().account(self.account);
        self.add_block(block_builder.build());
        self
    }

    pub fn legacy_open_from(mut self, send: &BlockEnum) -> Self {
        assert_eq!(send.destination_or_link(), self.account);
        let block_builder = BlockBuilder::legacy_open()
            .account(self.account)
            .source(send.hash());
        self.add_block(block_builder.build());
        self
    }

    pub fn legacy_receive_from(mut self, send: &BlockEnum) -> Self {
        assert_eq!(send.destination_or_link(), self.account);
        let block_builder = BlockBuilder::legacy_receive()
            .previous(self.frontier())
            .source(send.hash());
        self.add_block(block_builder.build());
        self
    }

    pub fn legacy_send(self) -> Self {
        self.legacy_send_with(|b| b)
    }

    pub fn legacy_send_with<F: FnMut(LegacySendBlockBuilder) -> LegacySendBlockBuilder>(
        mut self,
        mut f: F,
    ) -> Self {
        let block_builder = BlockBuilder::legacy_send()
            .account(self.account)
            .previous(self.frontier());
        self.add_block(f(block_builder).build());
        self
    }

    pub fn take_blocks(self) -> Vec<BlockEnum> {
        self.blocks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BlockType;

    #[test]
    fn default_account() {
        let builder = BlockChainBuilder::new();
        assert_eq!(builder.account, Account::from(42));
    }

    #[test]
    fn add_legacy_open() {
        let builder = BlockChainBuilder::for_account(1).legacy_open();
        let block = builder.latest_block();
        assert_eq!(block.account(), Account::from(1));
        assert_eq!(block.block_type(), BlockType::LegacyOpen);
        assert_eq!(block.sideband().unwrap().height, 1);
        assert_eq!(builder.frontier(), block.hash());
        assert_eq!(builder.height(), 1);
    }
}
