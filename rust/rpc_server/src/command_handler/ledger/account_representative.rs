use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{AccountRepresentativeDto, AccountRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn account_representative(
        &self,
        args: AccountRpcMessage,
    ) -> anyhow::Result<AccountRepresentativeDto> {
        let tx = self.node.ledger.read_txn();
        let account_info = self.load_account(&tx, &args.account)?;
        Ok(AccountRepresentativeDto::new(
            account_info.representative.as_account(),
        ))
    }
}
