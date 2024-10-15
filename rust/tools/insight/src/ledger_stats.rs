use rsnano_node::Node;

pub(crate) struct LedgerStats {
    pub total_blocks: u64,
    pub cemented_blocks: u64,
}

impl LedgerStats {
    pub(crate) fn new() -> Self {
        Self {
            total_blocks: 0,
            cemented_blocks: 0,
        }
    }

    pub(crate) fn update(&mut self, node: &Node) {
        self.total_blocks = node.ledger.block_count();
        self.cemented_blocks = node.ledger.cemented_count();
    }
}
