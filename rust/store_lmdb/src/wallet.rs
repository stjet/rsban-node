use std::sync::Mutex;

pub struct Wallet {
    pub representatives: Mutex<()>,
}

impl Wallet {
    pub fn new() -> Self {
        Self {
            representatives: Mutex::new(()),
        }
    }
}
