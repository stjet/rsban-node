use rsnano_rpc_messages::{AccountRpcMessage, KeyRpcMessage};

pub(crate) fn account_get(args: KeyRpcMessage) -> AccountRpcMessage {
    AccountRpcMessage::new(args.key.into())
}
