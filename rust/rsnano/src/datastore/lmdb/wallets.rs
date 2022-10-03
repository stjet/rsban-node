pub struct LmdbWallets {
    pub handle: u32,
}

impl LmdbWallets {
    pub fn new() -> Self {
        Self { handle: 0 }
    }
}
