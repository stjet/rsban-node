use rsnano_rpc_messages::{AccountResponse, KeyRpcMessage};

pub(crate) fn account_get(args: KeyRpcMessage) -> AccountResponse {
    AccountResponse::new(args.key.into())
}
