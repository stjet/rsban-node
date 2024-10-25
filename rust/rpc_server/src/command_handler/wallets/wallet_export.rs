use rsnano_rpc_messages::{JsonDto, WalletRpcMessage};
use serde_json::{to_string, Value};

pub(crate) fn wallet_export(args: WalletRpcMessage) -> JsonDto {
    JsonDto::new(Value::String(to_string(&args.wallet).unwrap()))
}
