use crate::command_handler::RpcCommandHandler;
use rsnano_core::{Account, BlockEnum, BlockHash};
use rsnano_node::NodeExt;
use rsnano_rpc_messages::{BlockHashesDto, ErrorDto, RpcDto, WalletWithCountArgs};
use std::{collections::VecDeque, time::Duration};

impl RpcCommandHandler {
    pub(crate) fn wallet_republish(&self, args: WalletWithCountArgs) -> RpcDto {
        if !self.enable_control {
            return RpcDto::Error(ErrorDto::RPCControlDisabled);
        }

        let accounts = match self.node.wallets.get_accounts_of_wallet(&args.wallet) {
            Ok(accounts) => accounts,
            Err(e) => return RpcDto::Error(ErrorDto::WalletsError(e)),
        };

        let (blocks, republish_bundle) = self.collect_blocks_to_republish(accounts, args.count);
        self.node.flood_block_many(
            republish_bundle.into(),
            Box::new(|| ()),
            Duration::from_millis(25),
        );
        RpcDto::WalletRepublish(BlockHashesDto::new(blocks))
    }

    fn collect_blocks_to_republish(
        &self,
        accounts: Vec<Account>,
        count: u64,
    ) -> (Vec<BlockHash>, VecDeque<BlockEnum>) {
        let mut blocks = Vec::new();
        let mut republish_bundle = VecDeque::new();
        let tx = self.node.ledger.read_txn();

        for account in accounts {
            let mut latest = self.node.ledger.any().account_head(&tx, &account).unwrap();
            let mut hashes = Vec::new();

            while !latest.is_zero() && hashes.len() < count as usize {
                hashes.push(latest);
                if let Some(block) = self.node.ledger.get_block(&tx, &latest) {
                    latest = block.previous();
                } else {
                    latest = BlockHash::zero();
                }
            }

            for hash in hashes.into_iter().rev() {
                if let Some(block) = self.node.ledger.get_block(&tx, &hash) {
                    republish_bundle.push_back(block);
                    blocks.push(hash);
                }
            }
        }

        (blocks, republish_bundle)
    }
}
