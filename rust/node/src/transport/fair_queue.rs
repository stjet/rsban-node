use std::sync::{Arc, Weak};

use super::ChannelEnum;

struct Origin<S> {
    source: S,
    channel: Option<Arc<ChannelEnum>>,
}

struct OriginEntry<S>
where
    S: Ord,
{
    source: S,

    // Optional is needed to distinguish between a source with no associated channel and a source with an expired channel
    // TODO: Store channel as shared_ptr after networking fixes are done
    maybe_channel: Option<Weak<ChannelEnum>>,
}

impl<S> OriginEntry<S>
where
    S: Ord,
{
    pub fn new(source: S) -> Self {
        Self {
            source,
            maybe_channel: None,
        }
    }

    pub fn new_with_channel(source: S, channel: &Arc<ChannelEnum>) -> Self {
        Self {
            source,
            maybe_channel: Some(Arc::downgrade(channel)),
        }
    }

    pub fn is_alive(&self) -> bool {
        if let Some(maybe_channel) = &self.maybe_channel {
            if let Some(channel) = maybe_channel.upgrade() {
                channel.is_alive()
            } else {
                false
            }
        } else {
            // Some sources (eg. local RPC) don't have an associated channel, never remove their queue
            true
        }
    }
}

impl<S> PartialEq for OriginEntry<S>
where
    S: Ord,
{
    fn eq(&self, other: &Self) -> bool {
        if self.source != other.source {
            return false;
        }

        match (self.maybe_channel.as_ref(), other.maybe_channel.as_ref()) {
            (None, None) => true,
            (Some(c1), Some(c2)) => c1.ptr_eq(c2),
            _ => false,
        }
    }
}

impl<S> Eq for OriginEntry<S> where S: Ord {}

impl<S> PartialOrd for OriginEntry<S>
where
    S: Ord,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let source_ordering = self.source.cmp(&other.source);
        if !matches!(source_ordering, std::cmp::Ordering::Equal) {
            return Some(source_ordering);
        }

        todo!()
        //match (self.maybe_channel.as_ref(), other.maybe_channel.as_ref()) {
        //    (None, None) => Some(std::cmp::Ordering::Equal),
        //    (Some(c1), Some(c2)) => c1.,
        //}
    }
}

//pub struct FairQueue<R, S> {}
