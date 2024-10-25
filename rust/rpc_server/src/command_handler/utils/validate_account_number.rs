use rsnano_rpc_messages::{RpcDto, SuccessDto};

pub fn validate_account_number(account: String) -> RpcDto {
    // TODO: actually validate!
    RpcDto::ValidateAccountNumber(SuccessDto::new())
}
