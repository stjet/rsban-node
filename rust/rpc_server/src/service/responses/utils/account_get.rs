use rsnano_rpc_messages::{AccountDto, AccountGetArgs, RpcDto};

pub async fn account_get(args: AccountGetArgs) -> RpcDto {
    RpcDto::AccountGet(AccountDto::new(args.key.into()))
}
