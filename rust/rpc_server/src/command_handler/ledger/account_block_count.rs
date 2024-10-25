use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{AccountBlockCountDto, AccountRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn account_block_count(
        &self,
        args: AccountRpcMessage,
    ) -> anyhow::Result<AccountBlockCountDto> {
        let tx = self.node.ledger.read_txn();
        let account_info = self.load_account(&tx, &args.account)?;
        Ok(AccountBlockCountDto::new(account_info.block_count))
    }
}
