use rsnano_core::PublicKey;
use rsnano_rpc_messages::AccountDto;
use serde_json::to_string_pretty;

pub async fn account_get(key: PublicKey) -> String {
    to_string_pretty(&AccountDto::new(key.into())).unwrap()
}
