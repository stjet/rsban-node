use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{AccountBlockCountDto, AccountRpcMessage, ErrorDto, RpcDto};

impl RpcCommandHandler {
    pub(crate) fn account_block_count(&self, args: AccountRpcMessage) -> RpcDto {
        let tx = self.node.ledger.read_txn();
        match self.node.ledger.store.account.get(&tx, &args.account) {
            Some(account_info) => {
                RpcDto::AccountBlockCount(AccountBlockCountDto::new(account_info.block_count))
            }
            None => RpcDto::Error(ErrorDto::AccountNotFound),
        }
    }
}
