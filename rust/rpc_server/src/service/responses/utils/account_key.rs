use rsnano_core::Account;
use serde_json::to_string_pretty;

pub async fn account_key(account: Account) -> String {
    to_string_pretty(&AccountKey::new(account.encode_hex())).unwrap()
}
