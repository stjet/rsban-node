use std::net::SocketAddr;

pub struct Socket {
    pub remote: Option<SocketAddr>,
}

impl Socket {
    pub fn new() -> Self {
        Self { remote: None }
    }
}
