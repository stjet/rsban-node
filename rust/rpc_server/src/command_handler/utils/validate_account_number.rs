use rsnano_rpc_messages::SuccessDto;

pub fn validate_account_number(account: String) -> SuccessDto {
    // TODO: actually validate!
    SuccessDto::new()
}
