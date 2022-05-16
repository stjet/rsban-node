use crate::{encode_hex, logger_mt::Logger, HardenedConstants};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(FromPrimitive)]
pub(crate) enum BootstrapMode {
    Legacy,
    Lazy,
    WalletLazy,
}

pub(crate) struct BootstrapAttempt {
    pub id: String,
    pub mode: BootstrapMode,
    next_log: Mutex<Instant>,
    logger: Arc<dyn Logger>,
}

impl BootstrapAttempt {
    pub(crate) fn new(logger: Arc<dyn Logger>, id: &str, mode: BootstrapMode) -> Self {
        let id = if id.is_empty() {
            encode_hex(HardenedConstants::get().random_128)
        } else {
            id.to_owned()
        };

        let result = Self {
            id,
            next_log: Mutex::new(Instant::now()),
            logger,
            mode,
        };

        result.start();
        result
    }

    fn start(&self){
        let mode = self.mode_text();
        let id = &self.id;
        self
            .logger
            .always_log(&format!("Starting {mode} bootstrap attempt with ID {id}"));
    }

    pub(crate) fn should_log(&self) -> bool {
        let mut next_log = self.next_log.lock().unwrap();
        let now = Instant::now();
        if *next_log < now {
            *next_log = now + Duration::from_secs(15);
            true
        } else {
            false
        }
    }

    pub(crate) fn mode_text(&self) -> &'static str {
        match self.mode {
            BootstrapMode::Legacy => "legacy",
            BootstrapMode::Lazy => "lazy",
            BootstrapMode::WalletLazy => "wallet_lazy",
        }
    }
}
