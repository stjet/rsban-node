use rsnano_rpc_messages::SuccessResponse;

pub fn validate_account_number(account: String) -> SuccessResponse {
    // TODO: actually validate!
    SuccessResponse::new()
}
