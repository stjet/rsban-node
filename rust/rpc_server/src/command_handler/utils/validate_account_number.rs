use rsnano_core::Account;
use rsnano_rpc_messages::{AccountCandidateArg, SuccessResponse, ValidResponse};

pub fn validate_account_number(args: AccountCandidateArg) -> ValidResponse {
    let valid = Account::decode_account(&args.account).is_ok();
    ValidResponse::new(valid)
}
