use anyhow::anyhow;
use rsnano_core::{Amount, MXRB_RATIO};
use rsnano_rpc_messages::AmountRpcMessage;

pub fn nano_to_raw(args: AmountRpcMessage) -> anyhow::Result<AmountRpcMessage> {
    if let Some(raw) = args.amount.number().checked_mul(*MXRB_RATIO) {
        Ok(AmountRpcMessage::new(Amount::raw(raw)))
    } else {
        Err(anyhow!("Invalid amount number"))
    }
}
