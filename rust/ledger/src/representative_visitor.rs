use rsnano_core::{
    Block, BlockHash, BlockVisitor, ChangeBlock, OpenBlock, ReceiveBlock, SendBlock, StateBlock,
};
use rsnano_store_traits::{Store, Transaction};

pub struct RepresentativeVisitor<'a> {
    txn: &'a dyn Transaction,
    store: &'a dyn Store,
    current: BlockHash,
    pub result: BlockHash,
}

impl<'a> RepresentativeVisitor<'a> {
    pub fn new(txn: &'a dyn Transaction, store: &'a dyn Store) -> Self {
        Self {
            txn,
            store,
            current: BlockHash::zero(),
            result: BlockHash::zero(),
        }
    }

    pub fn compute(&mut self, hash: BlockHash) {
        self.current = hash;
        while self.result.is_zero() {
            let block = self.store.block().get(self.txn, &self.current).unwrap();
            block.visit(self);
        }
    }
}

impl<'a> BlockVisitor for RepresentativeVisitor<'a> {
    fn send_block(&mut self, block: &SendBlock) {
        self.current = block.previous();
    }

    fn receive_block(&mut self, block: &ReceiveBlock) {
        self.current = block.previous();
    }

    fn open_block(&mut self, block: &OpenBlock) {
        self.result = block.hash();
    }

    fn change_block(&mut self, block: &ChangeBlock) {
        self.result = block.hash();
    }

    fn state_block(&mut self, block: &StateBlock) {
        self.result = block.hash();
    }
}
