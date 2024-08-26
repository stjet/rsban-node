use rsnano_core::{Account, Amount};
use rsnano_node::node::Node;
use rsnano_rpc_messages::AccountInfoDto;
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn account_info(
    node: Arc<Node>,
    account: Account,
    only_confirmed: Option<bool>,
    representative: Option<bool>,
    weight: Option<bool>,
    pending: Option<bool>,
) -> String {
    todo!()
}
