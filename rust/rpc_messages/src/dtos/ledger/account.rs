use rsnano_core::Account;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountDto {
    pub account: Account,
}

impl AccountDto {
    pub fn new(account: Account) -> Self {
        Self { account }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_str, to_string};

    #[test]
    fn serialize_account_dto() {
        let dto = AccountDto::new(1.into());

        let serialized = to_string(&dto).unwrap();

        let expected_json = serde_json::json!({
            "account": "nano_1111111111111111111111111111111111111111111111111113b8661hfk"
        });

        let actual_json: serde_json::Value = from_str(&serialized).unwrap();
        assert_eq!(actual_json, expected_json);
    }

    #[test]
    fn deserialize_account_balance_dto() {
        let json_str = r#"{
            "account": "nano_1111111111111111111111111111111111111111111111111113b8661hfk"
        }"#;

        let deserialized: AccountDto = from_str(json_str).unwrap();

        let expected = AccountDto::new(1.into());

        assert_eq!(deserialized, expected);
    }
}
