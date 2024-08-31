use rsnano_core::{Account, Amount, BlockHash};
use serde::{
    de::{self, MapAccess, Visitor},
    ser::SerializeMap,
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::fmt;

#[macro_export]
macro_rules! create_rpc_message {
    ($name:ident, $value_type:ty) => {
        #[derive(Debug, PartialEq, Eq)]
        pub struct $name {
            pub key: String,
            pub value: $value_type,
        }

        impl $name {
            pub fn new(key: String, value: $value_type) -> Self {
                Self { key, value }
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry(&self.key, &self.value)?;
                map.end()
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct DtoVisitor;

                impl<'de> Visitor<'de> for DtoVisitor {
                    type Value = $name;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("a map with a single key-value pair")
                    }

                    fn visit_map<A>(self, mut map: A) -> Result<$name, A::Error>
                    where
                        A: MapAccess<'de>,
                    {
                        let (key, value): (String, $value_type) = match map.next_entry()? {
                            Some(pair) => pair,
                            None => return Err(de::Error::invalid_length(0, &self)),
                        };

                        if map.next_entry::<String, $value_type>()?.is_some() {
                            return Err(de::Error::custom(
                                "Found more than one key-value pair in the map",
                            ));
                        }

                        Ok($name { key, value })
                    }
                }

                deserializer.deserialize_map(DtoVisitor)
            }
        }
    };
}

create_rpc_message!(BoolDto, bool);
create_rpc_message!(AccountRpcMessage, Account);
create_rpc_message!(AmountRpcMessage, Amount);
create_rpc_message!(BlockHashMessage, BlockHash);

#[cfg(test)]
mod tests {
    use crate::AccountRpcMessage;
    use rsnano_core::Account;
    use serde_json::{from_str, to_string_pretty};

    #[test]
    fn serialize_account_rpc_message() {
        assert_eq!(
            serde_json::to_string_pretty(&AccountRpcMessage::new(
                "account".to_string(),
                Account::from(123)
            ))
            .unwrap(),
            r#"{
  "account": "nano_111111111111111111111111111111111111111111111111115uwdgas549"
}"#
        )
    }

    #[test]
    fn derialize_account_rpc_message() {
        let account = Account::from(123);
        let account_arg = AccountRpcMessage::new("account".to_string(), account);
        let serialized = to_string_pretty(&account_arg).unwrap();
        let deserialized: AccountRpcMessage = from_str(&serialized).unwrap();
        assert_eq!(account_arg, deserialized)
    }
}
