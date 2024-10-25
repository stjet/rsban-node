use crate::command_handler::RpcCommandHandler;
use anyhow::bail;
use rsnano_core::{BlockDetails, BlockSubType, BlockType};
use rsnano_rpc_messages::{BlockInfoDto, HashRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn block_info(&self, args: HashRpcMessage) -> anyhow::Result<BlockInfoDto> {
        let txn = self.node.ledger.read_txn();
        let block = self.load_block_any(&txn, &args.hash)?;
        let account = block.account();

        let amount = self
            .node
            .ledger
            .any()
            .block_amount(&txn, &args.hash)
            .unwrap();

        let balance = self
            .node
            .ledger
            .any()
            .block_balance(&txn, &args.hash)
            .unwrap();

        let height = block.sideband().unwrap().height;
        let local_timestamp = block.sideband().unwrap().timestamp;
        let successor = block.sideband().unwrap().successor;

        let confirmed = self
            .node
            .ledger
            .confirmed()
            .block_exists_or_pruned(&txn, &args.hash);

        let contents = block.json_representation();

        let subtype = match block.block_type() {
            BlockType::State => serde_json::from_str::<BlockSubType>(&BlockDetails::state_subtype(
                &block.sideband().unwrap().details,
            ))
            .unwrap(),
            BlockType::LegacyChange => BlockSubType::Change,
            BlockType::LegacyOpen => BlockSubType::Open,
            BlockType::LegacySend => BlockSubType::Send,
            BlockType::LegacyReceive => BlockSubType::Receive,
            _ => bail!(Self::BLOCK_ERROR),
        };

        Ok(BlockInfoDto::new(
            account,
            amount,
            balance,
            height,
            local_timestamp,
            successor,
            confirmed,
            contents,
            subtype,
        ))
    }
}
