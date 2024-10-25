use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{AccountRpcMessage, HashRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn block_account(&self, args: HashRpcMessage) -> anyhow::Result<AccountRpcMessage> {
        let tx = self.node.ledger.read_txn();
        let block = self.load_block_any(&tx, &args.hash)?;
        Ok(AccountRpcMessage::new(block.account()))
    }
}
