use rsnano_core::Amount;
use rsnano_rpc_messages::{AmountRpcMessage, RpcDto};

pub async fn nano_to_raw(nano: Amount) -> RpcDto {
    RpcDto::NanoToRaw(AmountRpcMessage::new(Amount::raw(nano.number())))
}
