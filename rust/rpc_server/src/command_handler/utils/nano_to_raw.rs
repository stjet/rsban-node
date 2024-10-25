use rsnano_core::Amount;
use rsnano_rpc_messages::AmountRpcMessage;

pub fn nano_to_raw(args: AmountRpcMessage) -> AmountRpcMessage {
    AmountRpcMessage::new(Amount::raw(args.amount.number()))
}
