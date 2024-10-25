use rsnano_rpc_messages::{AccountRpcMessage, KeyRpcMessage, RpcDto};

pub(crate) fn account_key(args: AccountRpcMessage) -> RpcDto {
    RpcDto::AccountKey(KeyRpcMessage::new(args.account.into()))
}
