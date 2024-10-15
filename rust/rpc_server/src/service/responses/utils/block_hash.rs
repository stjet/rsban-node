use rsnano_core::{BlockEnum, JsonBlock};
use rsnano_rpc_messages::{HashRpcMessage, RpcDto};

pub async fn block_hash(block: JsonBlock) -> RpcDto {
    let block_enum: BlockEnum = block.into();
    RpcDto::BlockHash(HashRpcMessage::new(block_enum.hash()))
}
