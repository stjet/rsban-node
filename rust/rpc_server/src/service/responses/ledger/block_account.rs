use rsnano_core::BlockHash;
use rsnano_node::Node;
use rsnano_rpc_messages::{AccountRpcMessage, ErrorDto, RpcDto};
use std::sync::Arc;

pub async fn block_account(node: Arc<Node>, hash: BlockHash) -> RpcDto {
    let tx = node.ledger.read_txn();
    match &node.ledger.any().get_block(&tx, &hash) {
        Some(block) => {
            let account = block.account();
            let block_account = AccountRpcMessage::new(account);
            RpcDto::BlockAccount(block_account)
        }
        None => RpcDto::Error(ErrorDto::BlockNotFound)
    }
}
