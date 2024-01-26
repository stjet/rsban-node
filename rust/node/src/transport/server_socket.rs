use super::{ConnectionsPerAddress, Socket};
use std::sync::{Arc, Mutex};

pub struct ServerSocket {
    pub socket: Arc<Socket>,
    pub(crate) connections_per_address: Mutex<ConnectionsPerAddress>,
}
