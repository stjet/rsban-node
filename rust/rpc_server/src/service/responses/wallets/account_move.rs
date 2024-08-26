use rsnano_core::{Account, PublicKey, WalletId};
use rsnano_node::node::Node;
use rsnano_rpc_messages::AccountMoveDto;
use std::sync::Arc;
use toml::to_string_pretty;

pub async fn account_move(
    node: Arc<Node>,
    wallet: WalletId,
    source: WalletId,
    accounts: Vec<Account>,
) -> String {
    let mut account_move_dto = AccountMoveDto::new(true);
    let public_keys: Vec<PublicKey> = accounts.iter().map(|account| account.into()).collect();
    if node
        .wallets
        .move_accounts(&source, &wallet, &public_keys)
        .is_err()
    {
        account_move_dto.moved = false;
    }
    to_string_pretty(&account_move_dto).unwrap()
}
