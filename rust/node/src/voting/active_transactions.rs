use rsnano_core::BlockEnum;

pub struct ActiveTransactions {}

impl ActiveTransactions {
    pub fn new() -> Self {
        Self {}
    }

    pub fn erase(&self, _block: &BlockEnum) {}
}
