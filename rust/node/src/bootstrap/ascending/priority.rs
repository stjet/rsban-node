use ordered_float::OrderedFloat;
use std::ops::{Add, Deref, Div, Mul, Sub};

#[derive(PartialEq, Eq, Default, Clone, Copy, Ord, PartialOrd)]
pub struct Priority(OrderedFloat<f64>);

impl Priority {
    pub const fn new(value: f64) -> Self {
        Self(OrderedFloat(value))
    }

    pub const ZERO: Self = Self(OrderedFloat(0.0));
}

impl Add for Priority {
    type Output = Priority;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Priority {
    type Output = Priority;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Mul<f64> for Priority {
    type Output = Priority;

    fn mul(self, rhs: f64) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Div<f64> for Priority {
    type Output = Priority;

    fn div(self, rhs: f64) -> Self::Output {
        Self(self.0 / rhs)
    }
}

impl From<Priority> for f64 {
    fn from(value: Priority) -> Self {
        value.0 .0
    }
}

impl std::fmt::Debug for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0 .0, f)
    }
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0 .0, f)
    }
}

#[derive(PartialEq, Eq, Default, Clone, Copy)]
pub(crate) struct PriorityKeyDesc(pub Priority);

impl Ord for PriorityKeyDesc {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // order descending
        other.0.cmp(&self.0)
    }
}

impl PartialOrd for PriorityKeyDesc {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Deref for PriorityKeyDesc {
    type Target = Priority;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Priority> for PriorityKeyDesc {
    fn from(value: Priority) -> Self {
        Self(value)
    }
}
