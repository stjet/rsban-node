use crate::command_handler::RpcCommandHandler;
use anyhow::bail;
use rsnano_core::{BlockDetails, BlockHash, BlockSubType, BlockType};
use rsnano_rpc_messages::{BlockInfoDto, BlocksInfoDto, HashesArgs};
use std::collections::HashMap;

impl RpcCommandHandler {
    pub(crate) fn blocks_info(&self, args: HashesArgs) -> anyhow::Result<BlocksInfoDto> {
        let txn = self.node.ledger.read_txn();
        let mut blocks_info: HashMap<BlockHash, BlockInfoDto> = HashMap::new();

        for hash in args.hashes {
            let block = self.load_block_any(&txn, &hash)?;
            let account = block.account();
            let amount = self.node.ledger.any().block_amount(&txn, &hash).unwrap();
            let balance = self.node.ledger.any().block_balance(&txn, &hash).unwrap();
            let height = block.sideband().unwrap().height;
            let local_timestamp = block.sideband().unwrap().timestamp;
            let successor = block.sideband().unwrap().successor;
            let confirmed = self
                .node
                .ledger
                .confirmed()
                .block_exists_or_pruned(&txn, &hash);
            let contents = block.json_representation();

            let subtype = match block.block_type() {
                BlockType::State => serde_json::from_str::<BlockSubType>(
                    &BlockDetails::state_subtype(&block.sideband().unwrap().details),
                )
                .unwrap(),
                BlockType::LegacyChange => BlockSubType::Change,
                BlockType::LegacyOpen => BlockSubType::Open,
                BlockType::LegacySend => BlockSubType::Send,
                BlockType::LegacyReceive => BlockSubType::Receive,
                _ => bail!(Self::BLOCK_ERROR),
            };

            let block_info_dto = BlockInfoDto::new(
                account,
                amount,
                balance,
                height,
                local_timestamp,
                successor,
                confirmed,
                contents,
                subtype,
            );

            blocks_info.insert(hash, block_info_dto);
        }

        Ok(BlocksInfoDto::new(blocks_info))
    }
}
