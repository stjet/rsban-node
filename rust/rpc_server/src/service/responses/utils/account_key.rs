use rsnano_rpc_messages::{AccountKeyArgs, KeyDto, RpcDto};

pub async fn account_key(args: AccountKeyArgs) -> RpcDto {
    RpcDto::AccountKey(KeyDto::new(args.account.into()))
}
