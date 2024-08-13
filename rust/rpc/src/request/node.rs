use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "action")]
#[serde(rename_all = "snake_case")]
pub enum NodeRpcRequest {
    AccountBalance {
        account: String,
        only_confirmed: Option<bool>,
    },
}
