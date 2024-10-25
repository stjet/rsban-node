use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{AccountRepresentativeDto, AccountRpcMessage, ErrorDto, RpcDto};

impl RpcCommandHandler {
    pub(crate) fn account_representative(&self, args: AccountRpcMessage) -> RpcDto {
        let tx = self.node.ledger.read_txn();
        match self.node.ledger.store.account.get(&tx, &args.account) {
            Some(account_info) => RpcDto::AccountRepresentative(AccountRepresentativeDto::new(
                account_info.representative.as_account(),
            )),
            None => RpcDto::Error(ErrorDto::AccountNotFound),
        }
    }
}
