use rsnano_rpc_messages::{AccountKeyArgs, KeyRpcMessage, RpcDto};

pub async fn account_key(args: AccountKeyArgs) -> RpcDto {
    RpcDto::AccountKey(KeyRpcMessage::new(args.account.into()))
}
