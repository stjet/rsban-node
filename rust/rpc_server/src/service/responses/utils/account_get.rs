use rsnano_rpc_messages::{AccountRpcMessage, AccountGetArgs, RpcDto};

pub async fn account_get(args: AccountGetArgs) -> RpcDto {
    RpcDto::AccountGet(AccountRpcMessage::new(args.key.into()))
}
