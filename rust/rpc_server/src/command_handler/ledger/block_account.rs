use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{AccountRpcMessage, ErrorDto, HashRpcMessage, RpcDto};

impl RpcCommandHandler {
    pub(crate) fn block_account(&self, args: HashRpcMessage) -> RpcDto {
        let tx = self.node.ledger.read_txn();
        match &self.node.ledger.any().get_block(&tx, &args.hash) {
            Some(block) => {
                let account = block.account();
                let block_account = AccountRpcMessage::new(account);
                RpcDto::BlockAccount(block_account)
            }
            None => RpcDto::Error(ErrorDto::BlockNotFound),
        }
    }
}
