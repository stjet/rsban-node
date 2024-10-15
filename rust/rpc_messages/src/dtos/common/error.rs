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
    AccountNotFound,
    BlockNotFound,
    PeerNotFound,
    BlockError,
    MissingAccountInformation,
    WorkLow,
    GapPrevious,
    GapSource,
    Old,
    BadSignature,
    NegativeSpend,
    BalanceMismatch,
    Unreceivable,
    BlockPosition,
    GapEpochOpenPending,
    Fork,
    InsufficientWork,
    OpenedBurnAccount,
    Other,
    Stopped,
    NotStateBlock,
    LegacyBootstrapDisabled,
    LazyBootstrapDisabled,
    ConfirmationInfoNotFound,
    InvalidRoot,
    DifficultyOutOfRange,
    BlockRootMismatch,
    BlockWorkVersioMismatch,
    AccountHeadNotFound,
    InsufficientBalance
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
            ErrorDto2::BlockNotFound => "Block not found".to_string(),
            ErrorDto2::PeerNotFound => "Peer not found".to_string(),
            ErrorDto2::BlockError => "Block error".to_string(),
            ErrorDto2::MissingAccountInformation => "Missing account information".to_string(),
            ErrorDto2::WorkLow => "Work low".to_string(),
            ErrorDto2::GapPrevious => "Gap previous".to_string(),
            ErrorDto2::GapSource => "Gap source".to_string(),
            ErrorDto2::Old => "Old".to_string(),
            ErrorDto2::BadSignature => "Bad signature".to_string(),
            ErrorDto2::NegativeSpend => "Negative spend".to_string(),
            ErrorDto2::BalanceMismatch => "Balance mismatch".to_string(),
            ErrorDto2::Unreceivable => "Unreceivable".to_string(),
            ErrorDto2::BlockPosition => "Block position".to_string(),
            ErrorDto2::GapEpochOpenPending => "Gap epoch open pending".to_string(),
            ErrorDto2::Fork => "Fork".to_string(),
            ErrorDto2::InsufficientWork => "Insufficient work".to_string(),
            ErrorDto2::OpenedBurnAccount => "Opened burn account".to_string(),
            ErrorDto2::Other => "Other".to_string(),
            ErrorDto2::Stopped => "Stopped".to_string(),
            ErrorDto2::NotStateBlock => "Is not state block".to_string(),
            ErrorDto2::LegacyBootstrapDisabled => "Legacy boostrap is disabled".to_string(),
            ErrorDto2::LazyBootstrapDisabled => "Lazy boostrap is disabled".to_string(),
            ErrorDto2::ConfirmationInfoNotFound => "Confirmation info not found".to_string(),
            ErrorDto2::InvalidRoot => "Invalid root".to_string(),
            ErrorDto2::DifficultyOutOfRange => "Difficulty out of valid range".to_string(),
            ErrorDto2::BlockRootMismatch => "Block root mismatch".to_string(),
            ErrorDto2::BlockWorkVersioMismatch => "Block work version mismatch".to_string(),
            ErrorDto2::AccountHeadNotFound => "Account head not found".to_string(),
            ErrorDto2::InsufficientBalance => "Insufficient balance".to_string(),
        };

        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("error", &error_message)?;
        map.end()
    }
}

/*impl fmt::Display for ErrorDto2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let error_message = match self {
            Self::WalletsError(e) => e.to_string(),
            Self::RPCControlDisabled => "RPC control is disabled".to_string(),
            Self::AccountNotFound => "Account not found".to_string(),
            Self::BlockNotFound => "Block not found".to_string(),
            Self::PeerNotFound => "Peer not found".to_string(),
            Self::BlockError => "Block error".to_string(),
            Self::MissingAccountInformation => "Missing account information".to_string(),
        };
        write!(f, "{}", error_message)
    }
}

impl From<WalletsError> for ErrorDto {
    fn from(error: WalletsError) -> Self {
        ErrorDto::new(error.to_string())
    }
}*/

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
