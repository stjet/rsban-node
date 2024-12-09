use rsban_core::Amount;
use rsban_rpc_messages::AmountRpcMessage;

pub fn raw_to_nano(args: AmountRpcMessage) -> AmountRpcMessage {
    AmountRpcMessage::new(Amount::raw(args.amount.number() / Amount::nano(1).number()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_to_nano() {
        assert_eq!(
            raw_to_nano(AmountRpcMessage::new(Amount::raw(
                1_240_000_000_000_000_000_000_000_000_000
            )))
            .amount
            .number(),
            12
        );
    }
}
