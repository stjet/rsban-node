use crate::command_handler::RpcCommandHandler;
use rsnano_core::{BlockDetails, BlockSubType, BlockType};
use rsnano_rpc_messages::{BlockInfoDto, ErrorDto, HashRpcMessage, RpcDto};

impl RpcCommandHandler {
    pub(crate) fn block_info(&self, args: HashRpcMessage) -> RpcDto {
        let txn = self.node.ledger.read_txn();
        let block = if let Some(block) = self.node.ledger.get_block(&txn, &args.hash) {
            block
        } else {
            return RpcDto::Error(ErrorDto::BlockNotFound);
        };

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
            _ => return RpcDto::Error(ErrorDto::BlockError),
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

        RpcDto::BlockInfo(block_info_dto)
    }
}
