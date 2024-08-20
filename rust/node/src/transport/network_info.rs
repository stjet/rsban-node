use super::{ChannelDirection, ChannelId};
use rsnano_core::{utils::TEST_ENDPOINT_1, PublicKey};
use std::{
    collections::HashMap,
    net::SocketAddrV6,
    sync::{Arc, Mutex},
};

pub struct ChannelInfo {
    channel_id: ChannelId,
    peer_addr: SocketAddrV6,
    data: Mutex<ChannelInfoData>,
}

impl ChannelInfo {
    pub fn new(
        channel_id: ChannelId,
        peer_addr: SocketAddrV6,
        direction: ChannelDirection,
    ) -> Self {
        Self {
            channel_id,
            peer_addr,
            data: Mutex::new(ChannelInfoData {
                node_id: None,
                peering_addr: if direction == ChannelDirection::Outbound {
                    Some(peer_addr)
                } else {
                    None
                },
            }),
        }
    }

    pub fn new_test_instance() -> Self {
        Self::new(
            ChannelId::from(42),
            TEST_ENDPOINT_1,
            ChannelDirection::Outbound,
        )
    }

    pub fn channel_id(&self) -> ChannelId {
        self.channel_id
    }

    pub fn node_id(&self) -> Option<PublicKey> {
        self.data.lock().unwrap().node_id
    }

    /// The address that we are connected to. If this is an incoming channel, then
    /// the peer_addr uses an ephemeral port
    pub fn peer_addr(&self) -> SocketAddrV6 {
        self.peer_addr
    }

    /// The address where the peer accepts incoming connections. In case of an outbound
    /// channel, the peer_addr and peering_addr are the same
    pub fn peering_addr(&self) -> Option<SocketAddrV6> {
        self.data.lock().unwrap().peering_addr.clone()
    }

    pub fn peering_addr_or_peer_addr(&self) -> SocketAddrV6 {
        self.data
            .lock()
            .unwrap()
            .peering_addr
            .clone()
            .unwrap_or(self.peer_addr())
    }

    fn set_node_id(&self, node_id: PublicKey) {
        self.data.lock().unwrap().node_id = Some(node_id);
    }

    fn set_peering_addr(&self, peering_addr: SocketAddrV6) {
        self.data.lock().unwrap().peering_addr = Some(peering_addr);
    }
}

struct ChannelInfoData {
    node_id: Option<PublicKey>,
    peering_addr: Option<SocketAddrV6>,
}

pub struct NetworkInfo {
    next_channel_id: usize,
    channels: HashMap<ChannelId, Arc<ChannelInfo>>,
}

impl NetworkInfo {
    pub fn new() -> Self {
        Self {
            next_channel_id: 1,
            channels: HashMap::new(),
        }
    }

    pub fn add(
        &mut self,
        peer_addr: SocketAddrV6,
        direction: ChannelDirection,
    ) -> Arc<ChannelInfo> {
        let channel_id = self.get_next_channel_id();
        let channel_info = Arc::new(ChannelInfo::new(channel_id, peer_addr, direction));
        self.channels.insert(channel_id, channel_info.clone());
        channel_info
    }

    pub fn remove(&mut self, channel_id: ChannelId) {
        self.channels.remove(&channel_id);
    }

    pub fn set_node_id(&self, channel_id: ChannelId, node_id: PublicKey) {
        if let Some(channel) = self.channels.get(&channel_id) {
            channel.set_node_id(node_id);
        }
    }

    pub fn set_peering_addr(&self, channel_id: ChannelId, peering_addr: SocketAddrV6) {
        if let Some(channel) = self.channels.get(&channel_id) {
            channel.set_peering_addr(peering_addr);
        }
    }

    fn get_next_channel_id(&mut self) -> ChannelId {
        let id = self.next_channel_id.into();
        self.next_channel_id += 1;
        id
    }
}
