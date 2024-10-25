use rsnano_core::Amount;
use rsnano_rpc_messages::{AmountRpcMessage, RpcDto};

pub fn nano_to_raw(args: AmountRpcMessage) -> RpcDto {
    RpcDto::NanoToRaw(AmountRpcMessage::new(Amount::raw(args.amount.number())))
}
