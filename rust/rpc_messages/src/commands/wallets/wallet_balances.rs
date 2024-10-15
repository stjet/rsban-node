use std::collections::HashMap;
use crate::{AccountBalanceDto, RpcCommand};
use rsnano_core::{Account, Amount, WalletId};
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn wallet_balances(args: WalletBalancesArgs) -> Self {
        Self::WalletBalances(args)
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WalletBalancesArgs {
    pub wallet: WalletId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<Amount>,
}

impl WalletBalancesArgs {
    pub fn builder(wallet: WalletId) -> WalletBalancesArgsBuilder {
        WalletBalancesArgsBuilder::new(wallet)
    }
}

impl From<WalletId> for WalletBalancesArgs {
    fn from(wallet: WalletId) -> Self {
        Self {
            wallet,
            threshold: None,
        }
    }
}

pub struct WalletBalancesArgsBuilder {
    args: WalletBalancesArgs,
}

impl WalletBalancesArgsBuilder {
    fn new(wallet: WalletId) -> Self {
        Self {
            args: WalletBalancesArgs {
                wallet,
                threshold: None,
            },
        }
    }

    pub fn with_minimum_balance(mut self, threshold: Amount) -> Self {
        self.args.threshold = Some(threshold);
        self
    }

    pub fn build(self) -> WalletBalancesArgs {
        self.args
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WalletBalancesDto {
    pub balances: HashMap<Account, AccountBalanceDto>,
}

impl WalletBalancesDto {
    pub fn new(balances: HashMap<Account, AccountBalanceDto>) -> Self {
        Self { balances }
    }
}

#[cfg(test)]
mod tests {
    use crate::{RpcCommand, WalletBalancesArgs};
    use rsnano_core::{Amount, WalletId};
    use serde_json::to_string_pretty;
    use super::*;

    #[test]
    fn serialize_wallet_balances_command_threshold_none() {
        assert_eq!(
            to_string_pretty(&RpcCommand::wallet_balances(WalletId::zero().into())).unwrap(),
            r#"{
  "action": "wallet_balances",
  "wallet": "0000000000000000000000000000000000000000000000000000000000000000"
}"#
        )
    }

    #[test]
    fn serialize_wallet_balances_command_threshold_some() {
        let args = WalletBalancesArgs::builder(1.into())
            .with_minimum_balance(Amount::zero())
            .build();
        assert_eq!(
            to_string_pretty(&RpcCommand::wallet_balances(args)).unwrap(),
            r#"{
  "action": "wallet_balances",
  "wallet": "0000000000000000000000000000000000000000000000000000000000000001",
  "threshold": "0"
}"#
        )
    }

    #[test]
    fn deserialize_wallet_balances_command_threshold_none() {
        let cmd = RpcCommand::wallet_balances(WalletId::zero().into());
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }

    #[test]
    fn deserialize_wallet_balances_command_threshold_some() {
        let args = WalletBalancesArgs::builder(1.into())
            .with_minimum_balance(Amount::zero())
            .build();
        let cmd = RpcCommand::wallet_balances(args);
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }

    #[test]
    fn wallet_balances_args_builder() {
        let wallet = 1.into();
        let args = WalletBalancesArgs::builder(wallet)
            .with_minimum_balance(Amount::raw(1000))
            .build();

        assert_eq!(args.wallet, wallet);
        assert_eq!(args.threshold, Some(Amount::raw(1000)));
    }

    #[test]
    fn wallet_balances_args_from_wallet_id() {
        let wallet: WalletId = 1.into();
        let args: WalletBalancesArgs = wallet.into();

        assert_eq!(args.wallet, wallet);
        assert_eq!(args.threshold, None);
    }

    #[test]
    fn serialize_wallet_balances() {
        let mut balances = HashMap::new();
        let account1: Account = 1.into();
        let account2: Account = 2.into();

        let balance1 = AccountBalanceDto::new(Amount::raw(100), Amount::raw(50), Amount::raw(50));
        let balance2 = AccountBalanceDto::new(Amount::raw(200), Amount::raw(75), Amount::raw(75));

        balances.insert(account1.clone(), balance1);
        balances.insert(account2.clone(), balance2);

        let wallet_balances = WalletBalancesDto::new(balances);

        let serialized = serde_json::to_string(&wallet_balances).unwrap();

        let deserialized: WalletBalancesDto = serde_json::from_str(&serialized).unwrap();

        assert_eq!(wallet_balances, deserialized);
    }

    #[test]
    fn deserialize_wallet_balances() {
        let json_data = r#"{
            "balances": {
                "nano_1111111111111111111111111111111111111111111111111113b8661hfk": {"balance": "100", "pending": "50", "receivable": "50"},
                "nano_11111111111111111111111111111111111111111111111111147dcwzp3c": {"balance": "200", "pending": "75", "receivable": "75"}
            }
        }"#;

        let deserialized: WalletBalancesDto = serde_json::from_str(json_data).unwrap();

        let mut balances = HashMap::new();

        let account1: Account = 1.into();
        let account2: Account = 2.into();

        let balance1 = AccountBalanceDto::new(Amount::raw(100), Amount::raw(50), Amount::raw(50));
        let balance2 = AccountBalanceDto::new(Amount::raw(200), Amount::raw(75), Amount::raw(75));

        balances.insert(account1, balance1);
        balances.insert(account2, balance2);

        let expected_wallet_balances = WalletBalancesDto::new(balances);

        assert_eq!(deserialized, expected_wallet_balances);
    }
}
