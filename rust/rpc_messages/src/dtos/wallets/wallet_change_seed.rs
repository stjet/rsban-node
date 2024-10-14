use rsnano_core::Account;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WalletChangeSeedDto {
    pub success: String,
    pub last_restored_account: Account,
    pub restored_count: u32,
}

impl WalletChangeSeedDto {
    pub fn new(last_restored_account: Account, restored_count: u32) -> Self {
        Self {
            success: String::new(),
            last_restored_account,
            restored_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::Account;

    #[test]
    fn deserialize_wallet_change_seed_dto() {
        let json = r#"{"success":"","last_restored_account":"nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3","restored_count":15}"#;
        let deserialized: WalletChangeSeedDto = serde_json::from_str(json).unwrap();

        assert_eq!(deserialized.success, "");
        assert_eq!(
            deserialized.last_restored_account,
            Account::decode_account(
                "nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3"
            )
            .unwrap()
        );
        assert_eq!(deserialized.restored_count, 15);
    }
}
