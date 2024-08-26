mod account_create;

pub use account_create::*;
use serde_json::{json, to_string_pretty};

fn format_error_message(error: &str) -> String {
    let json_value = json!({ "error": error });
    to_string_pretty(&json_value).unwrap()
}
