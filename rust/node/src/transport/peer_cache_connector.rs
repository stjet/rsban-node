use crate::utils::Runnable;

// Tries to connect to peers that are stored in the peer cache
pub struct PeerCacheConnector {}

impl PeerCacheConnector {
    pub fn new() -> Self {
        Self {}
    }
}

impl Runnable for PeerCacheConnector {
    fn run(&mut self) {}
}
