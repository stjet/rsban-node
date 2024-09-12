use core::fmt;
use std::collections::HashMap;

use rsnano_core::{Account, Amount};
use serde::{de::{self, MapAccess, Visitor}, ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};
use crate::RpcCommand;

impl RpcCommand {
    pub fn representatives_online(weight: Option<bool>, accounts: Option<Vec<Account>>) -> Self {
        Self::RepresentativesOnline(RepresentativesOnlineArgs::new(weight, accounts))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepresentativesOnlineArgs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accounts: Option<Vec<Account>>,
}

impl RepresentativesOnlineArgs {
    pub fn new(weight: Option<bool>, accounts: Option<Vec<Account>>) -> Self {
        Self { weight, accounts }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RepresentativesOnlineDto {
    WithoutWeight(HashMap<Account, String>),
    WithWeight(HashMap<Account, Amount>),
}

impl Serialize for RepresentativesOnlineDto {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("representatives", &self.get_representatives_map())?;
        map.end()
    }
}

impl RepresentativesOnlineDto {
    pub fn new_without_weight(representatives: Vec<Account>) -> Self {
        let mut hashmap = HashMap::new();
        for representative in representatives {
            hashmap.insert(representative, String::new());
        }
        Self::WithoutWeight(hashmap)
    }

    pub fn new_with_weight(representatives: HashMap<Account, Amount>) -> Self {
        Self::WithWeight(representatives)
    }

    fn get_representatives_map(&self) -> HashMap<&Account, serde_json::Value> {
        match self {
            Self::WithoutWeight(reps) => reps.iter().map(|(k, _)| (k, serde_json::Value::String(String::new()))).collect(),
            Self::WithWeight(reps) => reps.iter().map(|(k, v)| (k, serde_json::json!({"weight": v.to_string_dec()}))).collect(),
        }
    }
}

impl<'de> Deserialize<'de> for RepresentativesOnlineDto {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RepresentativesOnlineDtoVisitor;

        impl<'de> Visitor<'de> for RepresentativesOnlineDtoVisitor {
            type Value = RepresentativesOnlineDto;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map with 'representatives' key")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut representatives = None;

                while let Some(key) = map.next_key::<String>()? {
                    if key == "representatives" {
                        let value: HashMap<Account, serde_json::Value> = map.next_value()?;
                        representatives = Some(value);
                    } else {
                        return Err(de::Error::unknown_field(&key, &["representatives"]));
                    }
                }

                let representatives = representatives.ok_or_else(|| de::Error::missing_field("representatives"))?;

                if representatives.values().all(|v| v.is_string()) {
                    Ok(RepresentativesOnlineDto::WithoutWeight(
                        representatives.into_iter().map(|(k, _)| (k, String::new())).collect()
                    ))
                } else {
                    Ok(RepresentativesOnlineDto::WithWeight(
                        representatives.into_iter().map(|(k, v)| {
                            let weight = v["weight"].as_str().unwrap_or_default();
                            (k, Amount::decode_dec(weight).unwrap_or_default())
                        }).collect()
                    ))
                }
            }
        }

        deserializer.deserialize_map(RepresentativesOnlineDtoVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialize_representatives_online_command_options_none() {
        let command = RpcCommand::representatives_online(None, None);
        let serialized = serde_json::to_value(command).unwrap();
        let expected = json!({"action": "representatives_online"});
        assert_eq!(serialized, expected);
    }

    #[test]
    fn deserialize_representatives_online_command_options_none() {
        let json = r#"{"action": "representatives_online"}"#;
        let deserialized: RpcCommand = serde_json::from_str(json).unwrap();
        let command = RpcCommand::representatives_online(None, None);
        assert!(matches!(deserialized, command));
    }

    #[test]
    fn serialize_representatives_online_command_options_some() {
        let weight = Amount::raw(1000);
        let accounts = vec![Account::decode_account("nano_1jg8zygjg3pp5w644emqcbmjqpnzmubfni3kfe1s8pooeuxsw49fdq1mco9j").unwrap()];
        let command = RpcCommand::representatives_online(Some(false), Some(accounts.clone()));
        let serialized = serde_json::to_value(command).unwrap();
        let expected = json!({
            "action": "representatives_online",
            "weight": "1000",
            "accounts": ["nano_1jg8zygjg3pp5w644emqcbmjqpnzmubfni3kfe1s8pooeuxsw49fdq1mco9j"]
        });
        assert_eq!(serialized, expected);
    }

    #[test]
    fn deserialize_representatives_online_command_options_some() {
        let json = r#"{
            "action": "representatives_online",
            "weight": "1000",
            "accounts": ["nano_1jg8zygjg3pp5w644emqcbmjqpnzmubfni3kfe1s8pooeuxsw49fdq1mco9j"]
        }"#;
        let deserialized: RpcCommand = serde_json::from_str(json).unwrap();
        if let RpcCommand::RepresentativesOnline(args) = deserialized {
            assert_eq!(args.weight, Some(false));
            assert_eq!(args.accounts, Some(vec![Account::decode_account("nano_1jg8zygjg3pp5w644emqcbmjqpnzmubfni3kfe1s8pooeuxsw49fdq1mco9j").unwrap()]));
        } else {
            panic!("Deserialized to wrong variant");
        }
    }

    #[test]
    fn serialize_representatives_online_dto_with_weight() {
        let mut representatives = HashMap::new();
        representatives.insert(
            Account::decode_account("nano_114nk4rwjctu6n6tr6g6ps61g1w3hdpjxfas4xj1tq6i8jyomc5d858xr1xi").unwrap(),
            Amount::raw(150462654614686936429917024683496890),
        );
        let dto = RepresentativesOnlineDto::new_with_weight(representatives);
        let serialized = serde_json::to_string(&dto).unwrap();
        let expected = r#"{"representatives":{"nano_114nk4rwjctu6n6tr6g6ps61g1w3hdpjxfas4xj1tq6i8jyomc5d858xr1xi":{"weight":"150462654614686936429917024683496890"}}}"#;
        assert_eq!(serialized, expected);
    }

    #[test]
    fn serialize_representatives_online_dto_without_weight() {
        let representative = Account::decode_account("nano_114nk4rwjctu6n6tr6g6ps61g1w3hdpjxfas4xj1tq6i8jyomc5d858xr1xi").unwrap();
        let dto = RepresentativesOnlineDto::new_without_weight(vec![representative]);
        let serialized = serde_json::to_string(&dto).unwrap();
        let expected = r#"{"representatives":{"nano_114nk4rwjctu6n6tr6g6ps61g1w3hdpjxfas4xj1tq6i8jyomc5d858xr1xi":""}}"#;
        assert_eq!(serialized, expected);
    }

    #[test]
    fn deserialize_representatives_online_dto_with_weight() {
        let json = r#"{"representatives":{"nano_114nk4rwjctu6n6tr6g6ps61g1w3hdpjxfas4xj1tq6i8jyomc5d858xr1xi":{"weight":"150462654614686936429917024683496890"}}}"#;
        let deserialized: RepresentativesOnlineDto = serde_json::from_str(json).unwrap();
        
        if let RepresentativesOnlineDto::WithWeight(reps) = deserialized {
            assert_eq!(reps.len(), 1);
            let account = Account::decode_account("nano_114nk4rwjctu6n6tr6g6ps61g1w3hdpjxfas4xj1tq6i8jyomc5d858xr1xi").unwrap();
            assert_eq!(reps[&account], Amount::raw(150462654614686936429917024683496890));
        } else {
            panic!("Deserialized to wrong variant");
        }
    }

    #[test]
    fn deserialize_representatives_online_dto_without_weight() {
        let json = r#"{"representatives":{"nano_114nk4rwjctu6n6tr6g6ps61g1w3hdpjxfas4xj1tq6i8jyomc5d858xr1xi":""}}"#;
        let deserialized: RepresentativesOnlineDto = serde_json::from_str(json).unwrap();
        
        if let RepresentativesOnlineDto::WithoutWeight(reps) = deserialized {
            assert_eq!(reps.len(), 1);
            let account = Account::decode_account("nano_114nk4rwjctu6n6tr6g6ps61g1w3hdpjxfas4xj1tq6i8jyomc5d858xr1xi").unwrap();
            assert_eq!(reps[&account], "");
        } else {
            panic!("Deserialized to wrong variant");
        }
    }
}