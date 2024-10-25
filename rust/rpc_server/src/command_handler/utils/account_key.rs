use rsnano_rpc_messages::{AccountArg, KeyRpcMessage};

pub(crate) fn account_key(args: AccountArg) -> KeyRpcMessage {
    KeyRpcMessage::new(args.account.into())
}
