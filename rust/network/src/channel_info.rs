use std::fmt::{Debug, Display};

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash)]
pub struct ChannelId(usize);

impl ChannelId {
    pub const LOOPBACK: Self = Self(0);
    pub const MIN: Self = Self(usize::MIN);
    pub const MAX: Self = Self(usize::MAX);

    pub fn as_usize(&self) -> usize {
        self.0
    }
}

impl Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl Debug for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

impl From<usize> for ChannelId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}
