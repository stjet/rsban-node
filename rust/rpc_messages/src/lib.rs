mod account_balance;
mod account_create;
mod account_info;
mod keepalive;
mod keypair;
mod receive;
mod send;
mod stop;
mod wallet_add;

pub use account_balance::*;
pub use account_create::*;
pub use account_info::*;
pub use keepalive::*;
pub use keypair::*;
pub use receive::*;
use rsnano_core::{Account, RawKey, WalletId};
pub use send::*;
use serde::{Deserialize, Serialize};
use std::net::Ipv6Addr;
pub use stop::*;
pub use wallet_add::*;

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RpcCommand {
    AccountBalance(AccountBalanceRequest),
    AccountCreate(AccountCreateRequest),
    AccountInfo(AccountInfoRequest),
    WalletAdd(WalletAddRequest),
    Receive(ReceiveRequest),
    Send(SendRequest),
    Keepalive(KeepaliveRequest),
    KeyCreate,
    WalletCreate,
    Stop,
}

impl RpcCommand {
    pub fn account_balance(account: Account, include_only_confirmed: Option<bool>) -> Self {
        Self::AccountBalance(AccountBalanceRequest {
            account,
            include_only_confirmed,
        })
    }

    pub fn account_create(wallet: WalletId, index: Option<u32>) -> Self {
        Self::AccountCreate(AccountCreateRequest { wallet, index })
    }

    pub fn account_info(account: Account) -> Self {
        Self::AccountInfo(AccountInfoRequest { account })
    }

    pub fn wallet_add(wallet_id: WalletId, key: RawKey) -> Self {
        Self::WalletAdd(WalletAddRequest {
            wallet: wallet_id,
            key,
        })
    }

    pub fn keepalive(address: Ipv6Addr, port: u16) -> Self {
        Self::Keepalive(KeepaliveRequest { address, port })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize() {
        let account = Account::from(123);
        let cmd = RpcCommand::account_info(account);
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
