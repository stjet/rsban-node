use once_cell::sync::Lazy;
use rand::Rng;
use rsnano_core::Account;

pub struct HardenedConstants {
    pub not_an_account: Account,
    pub random_128: u128,
}

impl HardenedConstants {
    pub fn get() -> &'static HardenedConstants {
        &INSTANCE
    }
}

static INSTANCE: Lazy<HardenedConstants> = Lazy::new(|| {
    let mut rng = rand::thread_rng();
    HardenedConstants {
        not_an_account: Account::from_bytes(rng.gen::<[u8; 32]>()),
        random_128: u128::from_ne_bytes(rng.gen::<[u8; 16]>()),
    }
});
