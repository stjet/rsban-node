use rsnano_core::{AccountInfo, Block, BlockHash};
use rsnano_store_lmdb::LmdbWriteTransaction;

use crate::Ledger;

use super::{
    instructions_executor::RollbackInstructionsExecutor, planner_factory::RollbackPlannerFactory,
    rollback_planner::RollbackStep,
};

pub(crate) struct BlockRollbackPerformer<'a> {
    ledger: &'a Ledger,
    pub txn: &'a mut LmdbWriteTransaction,
    pub rolled_back: Vec<Block>,
}

impl<'a> BlockRollbackPerformer<'a> {
    pub(crate) fn new(ledger: &'a Ledger, txn: &'a mut LmdbWriteTransaction) -> Self {
        Self {
            ledger,
            txn,
            rolled_back: Vec::new(),
        }
    }

    pub(crate) fn roll_back(mut self, block_hash: &BlockHash) -> anyhow::Result<Vec<Block>> {
        self.roll_back_block_and_successors(block_hash)?;
        Ok(self.rolled_back)
    }

    fn roll_back_block_and_successors(&mut self, block_hash: &BlockHash) -> anyhow::Result<()> {
        let block = self.load_block(block_hash)?;
        while self.block_exists(block_hash) {
            let head_block = self.load_account_head(&block)?;
            self.roll_back_head_block(head_block)?;
        }
        Ok(())
    }

    fn roll_back_head_block(&mut self, head_block: Block) -> Result<(), anyhow::Error> {
        let planner =
            RollbackPlannerFactory::new(self.ledger, self.txn, &head_block).create_planner()?;
        let step = planner.roll_back_head_block()?;
        self.execute(step, head_block)?;
        Ok(())
    }

    fn execute(&mut self, step: RollbackStep, head_block: Block) -> Result<(), anyhow::Error> {
        match step {
            RollbackStep::RollBackBlock(instructions) => {
                RollbackInstructionsExecutor::new(self.ledger, self.txn, &instructions).execute();
                self.rolled_back.push(head_block);
                Ok(())
            }
            RollbackStep::RequestDependencyRollback(dependency_hash) => {
                self.roll_back_block_and_successors(&dependency_hash)
            }
        }
    }

    fn block_exists(&self, block_hash: &BlockHash) -> bool {
        self.ledger.any().block_exists(self.txn, block_hash)
    }

    fn load_account_head(&self, block: &Block) -> anyhow::Result<Block> {
        let account_info = self.get_account_info(block);
        self.load_block(&account_info.head)
    }

    fn get_account_info(&self, block: &Block) -> AccountInfo {
        self.ledger
            .account_info(self.txn, &block.account())
            .unwrap()
    }

    fn load_block(&self, block_hash: &BlockHash) -> anyhow::Result<Block> {
        self.ledger
            .any()
            .get_block(self.txn, block_hash)
            .ok_or_else(|| anyhow!("block not found"))
    }
}