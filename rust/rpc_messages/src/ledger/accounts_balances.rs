use rsnano_core::{Account, Amount};
use crate::RpcCommand;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountsBalancesArgs {
    pub accounts: Vec<Account>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_only_confirmed: Option<bool>,
}

impl RpcCommand {
    pub fn accounts_balances(accounts: Vec<Account>, include_only_confirmed: Option<bool>) -> Self {
        RpcCommand::AccountsBalances(AccountsBalancesArgs {
            accounts,
            include_only_confirmed,
        })
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountsBalancesDto {
    pub balances: HashMap<Account, AccountBalanceDto>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountBalanceDto {
    pub balance: Amount,
    pub pending: Amount,
    pub receivable: Amount,
}

impl AccountBalanceDto {
    pub fn new(balance: Amount, pending: Amount, receivable: Amount) -> Self {
        Self {
            balance,
            pending,
            receivable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::Account;

    #[test]
    fn serialize_accounts_balances_dto() {
        let mut balances = HashMap::new();
        balances.insert(
            Account::decode_account("nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3").unwrap(),
            AccountBalanceDto {
                balance: Amount::raw(325586539664609129644855132177),
                pending: Amount::raw(2309372032769300000000000000000000),
                receivable: Amount::raw(2309372032769300000000000000000000),
            },
        );
        balances.insert(
            Account::decode_account("nano_3i1aq1cchnmbn9x5rsbap8b15akfh7wj7pwskuzi7ahz8oq6cobd99d4r3b7").unwrap(),
            AccountBalanceDto {
                balance: Amount::raw(10000000),
                pending: Amount::raw(0),
                receivable: Amount::raw(0),
            },
        );

        let dto = AccountsBalancesDto { balances };

        let serialized = serde_json::to_string(&dto).unwrap();
        let deserialized: AccountsBalancesDto = serde_json::from_str(&serialized).unwrap();

        assert_eq!(dto.balances.len(), deserialized.balances.len());
        assert_eq!(
            dto.balances.get(&Account::decode_account("nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3").unwrap()).unwrap().balance,
            Amount::raw(325586539664609129644855132177)
        );
        assert_eq!(
            dto.balances.get(&Account::decode_account("nano_3i1aq1cchnmbn9x5rsbap8b15akfh7wj7pwskuzi7ahz8oq6cobd99d4r3b7").unwrap()).unwrap().pending,
            Amount::raw(0)
        );
    }

    #[test]
    fn deserialize_accounts_balances_dto() {
        let json_data = r#"
        {
            "balances": {
                "nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3": {
                    "balance": "325586539664609129644855132177",
                    "pending": "2309372032769300000000000000000000",
                    "receivable": "2309372032769300000000000000000000"
                },
                "nano_3i1aq1cchnmbn9x5rsbap8b15akfh7wj7pwskuzi7ahz8oq6cobd99d4r3b7": {
                    "balance": "10000000",
                    "pending": "0",
                    "receivable": "0"
                }
            }
        }"#;

        let deserialized: AccountsBalancesDto = serde_json::from_str(json_data).unwrap();

        assert_eq!(deserialized.balances.len(), 2);

        let balance1 = deserialized.balances.get(&Account::decode_account("nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3").unwrap()).unwrap();
        assert_eq!(balance1.balance, Amount::raw(325586539664609129644855132177));
        assert_eq!(balance1.pending, Amount::raw(2309372032769300000000000000000000));
        assert_eq!(balance1.receivable, Amount::raw(2309372032769300000000000000000000));

        let balance2 = deserialized.balances.get(&Account::decode_account("nano_3i1aq1cchnmbn9x5rsbap8b15akfh7wj7pwskuzi7ahz8oq6cobd99d4r3b7").unwrap()).unwrap();
        assert_eq!(balance2.balance, Amount::raw(10000000));
        assert_eq!(balance2.pending, Amount::raw(0));
        assert_eq!(balance2.receivable, Amount::raw(0));
    }

    #[test]
    fn test_accounts_balances_rpc_command_serialization() {
        let accounts = vec![
            Account::decode_account("nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3").unwrap(),
            Account::decode_account("nano_3i1aq1cchnmbn9x5rsbap8b15akfh7wj7pwskuzi7ahz8oq6cobd99d4r3b7").unwrap(),
        ];

        // Test without include_only_confirmed
        let command = RpcCommand::accounts_balances(accounts.clone(), None);
        let serialized = serde_json::to_string(&command).unwrap();
        let expected = r#"{"action":"accounts_balances","accounts":["nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3","nano_3i1aq1cchnmbn9x5rsbap8b15akfh7wj7pwskuzi7ahz8oq6cobd99d4r3b7"]}"#;
        assert_eq!(serialized, expected);

        // Test with include_only_confirmed set to true
        let command = RpcCommand::accounts_balances(accounts, Some(true));
        let serialized = serde_json::to_string(&command).unwrap();
        let expected = r#"{"action":"accounts_balances","accounts":["nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3","nano_3i1aq1cchnmbn9x5rsbap8b15akfh7wj7pwskuzi7ahz8oq6cobd99d4r3b7"],"include_only_confirmed":true}"#;
        assert_eq!(serialized, expected);
    }

    #[test]
    fn test_accounts_balances_rpc_command_deserialization() {
        let json_data = r#"
        {
            "action": "accounts_balances",
            "accounts": [
                "nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3",
                "nano_3i1aq1cchnmbn9x5rsbap8b15akfh7wj7pwskuzi7ahz8oq6cobd99d4r3b7"
            ],
            "include_only_confirmed": true
        }"#;

        let deserialized: RpcCommand = serde_json::from_str(json_data).unwrap();

        match deserialized {
            RpcCommand::AccountsBalances(args) => {
                assert_eq!(args.accounts.len(), 2);
                assert_eq!(
                    args.accounts[0],
                    Account::decode_account("nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3").unwrap()
                );
                assert_eq!(
                    args.accounts[1],
                    Account::decode_account("nano_3i1aq1cchnmbn9x5rsbap8b15akfh7wj7pwskuzi7ahz8oq6cobd99d4r3b7").unwrap()
                );
                assert_eq!(args.include_only_confirmed, Some(true));
            },
            _ => panic!("Deserialized to wrong RpcCommand variant"),
        }

        // Test deserialization without include_only_confirmed
        let json_data_without_option = r#"
        {
            "action": "accounts_balances",
            "accounts": [
                "nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3",
                "nano_3i1aq1cchnmbn9x5rsbap8b15akfh7wj7pwskuzi7ahz8oq6cobd99d4r3b7"
            ]
        }"#;

        let deserialized: RpcCommand = serde_json::from_str(json_data_without_option).unwrap();

        match deserialized {
            RpcCommand::AccountsBalances(args) => {
                assert_eq!(args.accounts.len(), 2);
                assert_eq!(args.include_only_confirmed, None);
            },
            _ => panic!("Deserialized to wrong RpcCommand variant"),
        }
    }
}