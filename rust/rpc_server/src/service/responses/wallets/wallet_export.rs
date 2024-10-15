use rsnano_core::WalletId;
use rsnano_rpc_messages::{JsonDto, RpcDto};
use serde_json::{to_string, Value};

pub async fn wallet_export(wallet: WalletId) -> RpcDto {
    RpcDto::WalletExport(JsonDto::new(Value::String(to_string(&wallet).unwrap())))
}
