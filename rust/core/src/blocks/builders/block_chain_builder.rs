use crate::{
    Account, AccountInfo, Amount, BlockBuilder, BlockChainSection, BlockDetails, BlockEnum,
    BlockHash, BlockSideband, Epoch, KeyPair, DEV_GENESIS_KEY,
};

pub struct BlockChainBuilder {
    keypair: KeyPair,
    account: Account,
    balance: Amount,
    representative: Account,
    blocks: Vec<BlockEnum>,
}

impl BlockChainBuilder {
    pub fn new() -> Self {
        Self::with_keys(KeyPair::new())
    }

    pub fn genesis() -> Self {
        let mut result = Self::with_keys(DEV_GENESIS_KEY.clone());
        result.balance = Amount::MAX;
        result.add_block(
            BlockBuilder::legacy_open()
                .account(result.account)
                .source(BlockHash::zero())
                .sign(&result.keypair)
                .build(),
        );
        result
    }

    pub fn with_keys(keypair: KeyPair) -> Self {
        Self {
            account: keypair.public_key(),
            balance: Amount::zero(),
            blocks: Vec::new(),
            representative: Account::zero(),
            keypair,
        }
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

    pub fn legacy_open_from_account(&mut self, sender_chain: &BlockChainBuilder) -> &BlockEnum {
        self.legacy_open_from_account_block(sender_chain, sender_chain.height())
    }

    pub fn legacy_open_from_account_block(
        &mut self,
        sender_chain: &BlockChainBuilder,
        height: u64,
    ) -> &BlockEnum {
        let send_block = sender_chain.block(height);
        let amount = sender_chain.amount_of_block(height);
        assert_eq!(self.height(), 0);
        assert!(amount > Amount::zero());
        self.balance = amount;
        let open_block = BlockBuilder::legacy_open()
            .account(self.account)
            .source(send_block.hash())
            .sign(&self.keypair)
            .build();
        self.add_block(open_block)
    }

    pub fn legacy_receive_from_account(&mut self, sender_chain: &BlockChainBuilder) -> &BlockEnum {
        self.legacy_receive_from_account_block(sender_chain, sender_chain.height())
    }

    pub fn legacy_receive_from_self(&mut self) -> &BlockEnum {
        let send_block = self.block(self.height());
        let amount = self.amount_of_block(self.height());
        self.legacy_receive(send_block.hash(), amount)
    }

    pub fn legacy_receive_from_account_block(
        &mut self,
        sender: &BlockChainBuilder,
        height: u64,
    ) -> &BlockEnum {
        let send_block = sender.block(height);
        let amount = sender.amount_of_block(height);
        self.legacy_receive(send_block.hash(), amount)
    }

    fn legacy_receive(&mut self, source: BlockHash, amount: Amount) -> &BlockEnum {
        assert!(amount > Amount::zero());
        let block_builder = BlockBuilder::legacy_receive()
            .previous(self.frontier())
            .source(source)
            .sign(&self.keypair);
        self.balance += amount;
        self.add_block(block_builder.build())
    }

    pub fn legacy_send(&mut self) -> &BlockEnum {
        self.legacy_send_to(Account::from(42), Amount::raw(1))
    }

    pub fn legacy_send_to(&mut self, destination: Account, amount: Amount) -> &BlockEnum {
        let new_balance = self.balance - amount;
        let block = BlockBuilder::legacy_send()
            .account(self.account)
            .previous(self.frontier())
            .destination(destination)
            .balance(new_balance)
            .sign(self.keypair.clone())
            .build();
        self.add_block(block)
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

    fn amount_of_block(&self, height: u64) -> Amount {
        let balance = self.balance_on_height(height);
        let previous_balance = self.balance_on_height(height - 1);
        if balance > previous_balance {
            balance - previous_balance
        } else {
            previous_balance - balance
        }
    }

    fn balance_on_height(&self, height: u64) -> Amount {
        if height == 0 {
            Amount::zero()
        } else {
            self.blocks[height as usize - 1].balance_calculated()
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BlockType;

    #[test]
    fn default_account() {
        let chain1 = BlockChainBuilder::new();
        let chain2 = BlockChainBuilder::new();
        assert_ne!(chain1.account, chain2.account);
    }

    #[test]
    fn add_legacy_open() {
        let mut genesis = BlockChainBuilder::genesis();
        let mut chain = BlockChainBuilder::new();
        genesis.legacy_send_to(chain.account, Amount::raw(10));
        chain.legacy_open_from_account(&genesis);
        let block = chain.latest_block();
        assert_eq!(block.account(), chain.account());
        assert_eq!(block.block_type(), BlockType::LegacyOpen);
        assert_eq!(block.sideband().unwrap().height, 1);
        assert_eq!(chain.frontier(), block.hash());
        assert_eq!(chain.height(), 1);
        assert_eq!(
            chain.account_info().representative,
            block.representative().unwrap()
        );
    }
}
