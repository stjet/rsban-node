use std::fmt;

use rsnano_node::wallets::WalletsError;
use serde::{ser::SerializeMap, Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ErrorDto {
    pub error: String,
}

impl ErrorDto {
    pub fn new(error: String) -> Self {
        Self { error }
    }
}

#[derive(Debug, Deserialize)]
pub enum ErrorDto2 {
    WalletsError(WalletsError),
    RPCControlDisabled,
    AccountNotFound
}

impl Serialize for ErrorDto2 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let error_message = match self {
            ErrorDto2::WalletsError(e) => e.to_string(),
            ErrorDto2::RPCControlDisabled => "RPC control is disabled".to_string(),
            ErrorDto2::AccountNotFound => "Account not found".to_string(),
        };

        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("error", &error_message)?;
        map.end()
    }
}

impl fmt::Display for ErrorDto2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let error_message = match self {
            Self::WalletsError(e) => e.to_string(),
            Self::RPCControlDisabled => "RPC control is disabled".to_string(),
            Self::AccountNotFound => "Account not found".to_string(),
        };
        write!(f, "{}", error_message)
    }
}

impl From<WalletsError> for ErrorDto {
    fn from(error: WalletsError) -> Self {
        ErrorDto::new(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn serialize_error_dto() {
        let error_dto = ErrorDto::new("An error occurred".to_string());
        let serialized = serde_json::to_string(&error_dto).unwrap();
        let expected_json = r#"{"error":"An error occurred"}"#;
        assert_eq!(serialized, expected_json);
    }

    #[test]
    fn deserialize_error_dto() {
        let json_str = r#"{"error":"An error occurred"}"#;
        let deserialized: ErrorDto = serde_json::from_str(json_str).unwrap();
        let expected_error_dto = ErrorDto::new("An error occurred".to_string());
        assert_eq!(deserialized, expected_error_dto);
    }
}
