use rsnano_core::{Amount, MXRB_RATIO};
use rsnano_rpc_messages::AmountRpcMessage;

pub fn raw_to_nano(args: AmountRpcMessage) -> AmountRpcMessage {
    AmountRpcMessage::new(Amount::nano(args.amount.number() / *MXRB_RATIO))
}
