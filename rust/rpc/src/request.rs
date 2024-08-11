use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "action")]
#[serde(rename_all = "snake_case")]
pub(crate) enum RpcRequest {
    AccountBalance {
        account: String,
        only_confirmed: Option<bool>,
    },
    #[serde(other)]
    UnknownCommand,
}
