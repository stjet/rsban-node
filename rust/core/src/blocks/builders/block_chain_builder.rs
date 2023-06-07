use crate::{
    Account, AccountInfo, Amount, BlockBuilder, BlockChainSection, BlockDetails, BlockEnum,
    BlockHash, BlockSideband, Epoch, LegacySendBlockBuilder,
};

pub struct BlockChainBuilder {
    account: Account,
    balance: Amount,
    representative: Account,
    blocks: Vec<BlockEnum>,
}

impl BlockChainBuilder {
    pub fn new() -> Self {
        Self::for_account(42)
    }

    pub fn for_account<T: Into<Account>>(account: T) -> Self {
        Self {
            account: account.into(),
            balance: Amount::zero(),
            blocks: Vec::new(),
            representative: Account::zero(),
        }
    }

    pub fn from_send_block(block: &BlockEnum) -> Self {
        Self::from_send_block_with_amount(block, Amount::raw(10))
    }

    pub fn from_send_block_with_amount(block: &BlockEnum, amount: Amount) -> Self {
        let BlockEnum::LegacySend(send_block) = block else {
            panic!("not a send block!")
        };

        Self::for_account(*send_block.mandatory_destination()).legacy_open_from(block, amount)
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

    pub fn block(&self, height: u64) -> &BlockEnum {
        &self.blocks[height as usize - 1]
    }

    pub fn latest_block(&self) -> &BlockEnum {
        self.blocks.last().unwrap()
    }

    fn add_block(&mut self, mut block: BlockEnum) -> &BlockEnum {
        if let Some(new_balance) = block.balance_opt() {
            self.balance = new_balance;
        }

        block.set_sideband(BlockSideband {
            height: self.height() + 1,
            timestamp: 1,
            successor: BlockHash::zero(),
            account: self.account,
            balance: self.balance,
            details: BlockDetails::new(Epoch::Epoch0, false, false, false),
            source_epoch: Epoch::Epoch0,
        });

        if self.blocks.len() > 0 {
            let previous = self.blocks.last_mut().unwrap();
            let mut sideband = previous.sideband().unwrap().clone();
            sideband.successor = block.hash();
            previous.set_sideband(sideband);
        }

        if let Some(rep) = block.representative() {
            self.representative = rep;
        }

        self.blocks.push(block);
        self.blocks.last().unwrap()
    }

    pub fn legacy_open(mut self) -> Self {
        let block_builder = BlockBuilder::legacy_open().account(self.account);
        self.add_block(block_builder.build());
        self
    }

    pub fn legacy_open_from(mut self, send: &BlockEnum, amount: Amount) -> Self {
        assert_eq!(send.destination_or_link(), self.account);
        let block_builder = BlockBuilder::legacy_open()
            .account(self.account)
            .source(send.hash());
        self.balance += amount;
        self.add_block(block_builder.build());
        self
    }

    pub fn legacy_receive_from(mut self, send: &BlockEnum, amount: Amount) -> Self {
        assert_eq!(send.destination_or_link(), self.account);
        let block_builder = BlockBuilder::legacy_receive()
            .previous(self.frontier())
            .source(send.hash());
        self.balance += amount;
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

    pub fn section(&self, bottom: u64, top: u64) -> BlockChainSection {
        BlockChainSection {
            account: self.account(),
            bottom_hash: self.blocks[bottom as usize - 1].hash(),
            bottom_height: bottom,
            top_hash: self.blocks[top as usize - 1].hash(),
            top_height: top,
        }
    }

    pub fn frontier_section(&self) -> BlockChainSection {
        self.section(self.height(), self.height())
    }

    pub fn account_info(&self) -> AccountInfo {
        AccountInfo {
            head: self.frontier(),
            representative: self.representative,
            open_block: self.open(),
            balance: self.latest_block().balance_calculated(),
            modified: 123,
            block_count: self.height(),
            epoch: self.latest_block().sideband().unwrap().details.epoch,
        }
    }

    pub fn open_last_destination(&self) -> BlockChainBuilder {
        let latest = self.latest_block();
        let previous_balance = self.balance_on_height(self.height() - 1);
        let amount_sent = latest.balance_calculated() - previous_balance;
        BlockChainBuilder::from_send_block_with_amount(latest, amount_sent)
    }

    fn balance_on_height(&self, height: u64) -> Amount {
        if height == 0 {
            Amount::zero()
        } else {
            self.blocks[height as usize - 1].balance_calculated()
        }
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
        assert_eq!(
            builder.account_info().representative,
            block.representative().unwrap()
        );
    }
}
