mod config;
mod request;
mod response;
mod server;
mod toml;

pub use config::*;
use serde_json::{json, to_string_pretty};
pub use server::run_rpc_server;
pub use toml::*;

pub(crate) fn format_error_message(error: &str) -> String {
    let json_value = json!({ "error": error });
    to_string_pretty(&json_value).unwrap()
}
