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

#[derive(Copy, Clone, PartialEq, Eq, Default, PartialOrd, Ord)]
pub struct RpcU64(u64);

impl RpcU64 {
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl From<u64> for RpcU64 {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<RpcU64> for u64 {
    fn from(value: RpcU64) -> Self {
        value.0
    }
}

impl Debug for RpcU64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Serialize for RpcU64 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for RpcU64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(U64Visitor {})
    }
}

struct U64Visitor {}

impl<'de> Visitor<'de> for U64Visitor {
    type Value = RpcU64;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("u64")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let value =
            u64::from_str_radix(v, 10).map_err(|_| serde::de::Error::custom("expected u64"))?;
        Ok(value.into())
    }
}

/// Bool expressed as "1"=true and "0"=false
#[derive(Copy, Clone, PartialEq, Eq, Default)]
pub struct RpcBoolNumber(bool);

impl From<bool> for RpcBoolNumber {
    fn from(value: bool) -> Self {
        Self(value)
    }
}

impl From<RpcBoolNumber> for bool {
    fn from(value: RpcBoolNumber) -> Self {
        value.0
    }
}

impl Debug for RpcBoolNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Serialize for RpcBoolNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(if self.0 { "1" } else { "0" })
    }
}

impl<'de> Deserialize<'de> for RpcBoolNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let result = deserializer.deserialize_str(BoolVisitor {})?;
        Ok(RpcBoolNumber(result))
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Default)]
pub struct RpcBool(bool);

impl From<bool> for RpcBool {
    fn from(value: bool) -> Self {
        Self(value)
    }
}

impl From<RpcBool> for bool {
    fn from(value: RpcBool) -> Self {
        value.0
    }
}

impl Debug for RpcBool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Serialize for RpcBool {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(if self.0 { "true" } else { "false" })
    }
}

impl<'de> Deserialize<'de> for RpcBool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let result = deserializer.deserialize_str(BoolVisitor {})?;
        Ok(RpcBool(result))
    }
}

struct BoolVisitor {}

impl<'de> Visitor<'de> for BoolVisitor {
    type Value = bool;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("bool")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        match v {
            "1" | "true" => Ok(true),
            "0" | "false" => Ok(false),
            _ => Err(serde::de::Error::custom("bool expected")),
        }
    }
}

pub fn unwrap_u64_or_max(i: Option<RpcU64>) -> u64 {
    i.unwrap_or(u64::MAX.into()).into()
}

pub fn unwrap_u64_or_zero(i: Option<RpcU64>) -> u64 {
    i.unwrap_or_default().into()
}

pub fn unwrap_bool_or_false(i: Option<RpcBool>) -> bool {
    i.unwrap_or_default().into()
}

pub fn unwrap_bool_or_true(i: Option<RpcBool>) -> bool {
    i.unwrap_or(true.into()).into()
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
        let value: RpcU16 = serde_json::from_str("\"123\"").unwrap();
        assert_eq!(value, 123.into());
    }

    #[test]
    fn u64_serialize() {
        let value = RpcU64::from(123);
        assert_eq!(format!("{:?}", value), "123");
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "\"123\"");
    }

    #[test]
    fn u64_deserialize() {
        let value: RpcU64 = serde_json::from_str("\"123\"").unwrap();
        assert_eq!(value, 123.into());
    }

    #[test]
    fn bool_number_serialize() {
        let true_value = RpcBoolNumber::from(true);
        let false_value = RpcBoolNumber::from(false);
        assert_eq!(format!("{:?}", true_value), "true");
        assert_eq!(format!("{:?}", false_value), "false");
        let json = serde_json::to_string(&true_value).unwrap();
        assert_eq!(json, "\"1\"");
        let json = serde_json::to_string(&false_value).unwrap();
        assert_eq!(json, "\"0\"");
    }

    #[test]
    fn bool_number_deserialize() {
        let a: RpcBoolNumber = serde_json::from_str("\"1\"").unwrap();
        let b: RpcBoolNumber = serde_json::from_str("\"0\"").unwrap();
        let c: RpcBoolNumber = serde_json::from_str("\"true\"").unwrap();
        let d: RpcBoolNumber = serde_json::from_str("\"false\"").unwrap();
        assert_eq!(a, true.into());
        assert_eq!(b, false.into());
        assert_eq!(c, true.into());
        assert_eq!(d, false.into());
    }

    #[test]
    fn bool_serialize() {
        let true_value = RpcBool::from(true);
        let false_value = RpcBool::from(false);
        assert_eq!(format!("{:?}", true_value), "true");
        assert_eq!(format!("{:?}", false_value), "false");
        let json = serde_json::to_string(&true_value).unwrap();
        assert_eq!(json, "\"true\"");
        let json = serde_json::to_string(&false_value).unwrap();
        assert_eq!(json, "\"false\"");
    }

    #[test]
    fn bool_deserialize() {
        let a: RpcBoolNumber = serde_json::from_str("\"1\"").unwrap();
        let b: RpcBoolNumber = serde_json::from_str("\"0\"").unwrap();
        let c: RpcBoolNumber = serde_json::from_str("\"true\"").unwrap();
        let d: RpcBoolNumber = serde_json::from_str("\"false\"").unwrap();
        assert_eq!(a, true.into());
        assert_eq!(b, false.into());
        assert_eq!(c, true.into());
        assert_eq!(d, false.into());
    }
}
