use rsnano_core::{Account, Amount};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepresentativesOnlineDto {
    pub representatives: HashMap<Account, Option<Amount>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_representatives_online_dto_with_weight() {
        let mut representatives = HashMap::new();
        representatives.insert(
            Account::decode_account(
                "nano_114nk4rwjctu6n6tr6g6ps61g1w3hdpjxfas4xj1tq6i8jyomc5d858xr1xi",
            )
            .unwrap(),
            Some(Amount::raw(150462654614686936429917024683496890)),
        );
        let dto = RepresentativesOnlineDto { representatives };
        let serialized = serde_json::to_string(&dto).unwrap();
        let expected = r#"{"representatives":{"nano_114nk4rwjctu6n6tr6g6ps61g1w3hdpjxfas4xj1tq6i8jyomc5d858xr1xi":"150462654614686936429917024683496890"}}"#;
        assert_eq!(serialized, expected);
    }

    #[test]
    fn serialize_representatives_online_dto_without_weight() {
        let mut representatives = HashMap::new();
        representatives.insert(
            Account::decode_account(
                "nano_114nk4rwjctu6n6tr6g6ps61g1w3hdpjxfas4xj1tq6i8jyomc5d858xr1xi",
            )
            .unwrap(),
            None,
        );
        let dto = RepresentativesOnlineDto { representatives };
        let serialized = serde_json::to_string(&dto).unwrap();
        let expected = r#"{"representatives":{"nano_114nk4rwjctu6n6tr6g6ps61g1w3hdpjxfas4xj1tq6i8jyomc5d858xr1xi":null}}"#;
        assert_eq!(serialized, expected);
    }

    #[test]
    fn deserialize_representatives_online_dto() {
        let json = r#"{"representatives":{"nano_114nk4rwjctu6n6tr6g6ps61g1w3hdpjxfas4xj1tq6i8jyomc5d858xr1xi":"150462654614686936429917024683496890"}}"#;
        let deserialized: RepresentativesOnlineDto = serde_json::from_str(json).unwrap();

        assert_eq!(deserialized.representatives.len(), 1);
        let account = Account::decode_account(
            "nano_114nk4rwjctu6n6tr6g6ps61g1w3hdpjxfas4xj1tq6i8jyomc5d858xr1xi",
        )
        .unwrap();
        assert_eq!(
            deserialized.representatives[&account],
            Some(Amount::raw(150462654614686936429917024683496890))
        );
    }

    #[test]
    fn deserialize_representatives_online_dto_without_weight() {
        let json = r#"{"representatives":{"nano_114nk4rwjctu6n6tr6g6ps61g1w3hdpjxfas4xj1tq6i8jyomc5d858xr1xi":null}}"#;
        let deserialized: RepresentativesOnlineDto = serde_json::from_str(json).unwrap();

        assert_eq!(deserialized.representatives.len(), 1);
        let account = Account::decode_account(
            "nano_114nk4rwjctu6n6tr6g6ps61g1w3hdpjxfas4xj1tq6i8jyomc5d858xr1xi",
        )
        .unwrap();
        assert_eq!(deserialized.representatives[&account], None);
    }
}
