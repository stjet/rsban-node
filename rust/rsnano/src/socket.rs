use std::net::SocketAddr;

pub struct Socket {
	/// The other end of the connection
    pub remote: Option<SocketAddr>,
}

impl Socket {
    pub fn new() -> Self {
        Self { remote: None }
    }

    pub fn async_connect(&mut self, endpoint: SocketAddr) {
        self.remote = Some(endpoint);
    }
}
