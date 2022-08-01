pub struct TcpMessageItem {}

pub struct TcpMessageManager {
    max_entries: usize,
}

impl TcpMessageManager {
    pub fn new(incoming_connections_max: usize) -> Self {
        Self {
            max_entries: incoming_connections_max * MAX_ENTRIES_PER_CONNECTION + 1,
        }
    }
}

const MAX_ENTRIES_PER_CONNECTION: usize = 16;
