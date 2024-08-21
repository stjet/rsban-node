use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "action")]
#[serde(rename_all = "snake_case")]
pub(crate) enum WalletRpcRequest {
    #[serde(other)]
    UnknownCommand,
}
