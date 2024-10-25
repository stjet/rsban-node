use rsnano_rpc_messages::{AccountRpcMessage, KeyRpcMessage};

pub(crate) fn account_key(args: AccountRpcMessage) -> KeyRpcMessage {
    KeyRpcMessage::new(args.account.into())
}
