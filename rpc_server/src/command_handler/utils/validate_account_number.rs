use rsban_core::Account;
use rsban_rpc_messages::{AccountCandidateArg, ValidResponse};

pub fn validate_account_number(args: AccountCandidateArg) -> ValidResponse {
    let valid = Account::decode_account(&args.account).is_ok();
    ValidResponse::new(valid)
}
