use serde::{de::Visitor, Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct RpcU16(u16);

impl From<u16> for RpcU16 {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl From<RpcU16> for u16 {
    fn from(value: RpcU16) -> Self {
        value.0
    }
}

impl Debug for RpcU16 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Serialize for RpcU16 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for RpcU16 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(U16Visitor {})
    }
}

struct U16Visitor {}

impl<'de> Visitor<'de> for U16Visitor {
    type Value = RpcU16;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("u16")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let value =
            u16::from_str_radix(v, 10).map_err(|_| serde::de::Error::custom("expected u16"))?;
        Ok(value.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u16_serialize() {
        let value = RpcU16::from(123);
        assert_eq!(format!("{:?}", value), "123");
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "\"123\"");
    }

    #[test]
    fn u16_deserialize() {
        assert_eq!(42, serde_json::from_str::<u16>("42").unwrap());
        let value: RpcU16 = serde_json::from_str("\"123\"").unwrap();
        assert_eq!(value, 123.into());
    }
}
