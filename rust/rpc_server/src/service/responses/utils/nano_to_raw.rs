use rsnano_core::Amount;
use rsnano_rpc_messages::AmountRpcMessage;
use serde_json::to_string_pretty;

pub async fn nano_to_raw(nano: Amount) -> String {
    to_string_pretty(&AmountRpcMessage::new(Amount::raw(nano.number()))).unwrap()
}
