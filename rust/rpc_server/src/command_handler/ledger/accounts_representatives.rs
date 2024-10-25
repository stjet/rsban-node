use crate::command_handler::RpcCommandHandler;
use rsnano_core::Account;
use rsnano_rpc_messages::{AccountsRepresentativesDto, AccountsRpcMessage};
use std::collections::HashMap;

impl RpcCommandHandler {
    pub(crate) fn accounts_representatives(
        &self,
        args: AccountsRpcMessage,
    ) -> AccountsRepresentativesDto {
        let tx = self.node.ledger.read_txn();
        let mut representatives: HashMap<Account, Account> = HashMap::new();
        let mut errors: HashMap<Account, String> = HashMap::new();

        for account in args.accounts {
            match self.node.ledger.store.account.get(&tx, &account) {
                Some(account_info) => {
                    representatives.insert(account, account_info.representative.as_account());
                }
                None => {
                    errors.insert(account, "Account not found".to_string());
                }
            }
        }

        let mut dto = AccountsRepresentativesDto::new(representatives);
        if !errors.is_empty() {
            dto.errors = Some(errors);
        }

        dto
    }
}
