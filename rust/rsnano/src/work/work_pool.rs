use std::{
    sync::atomic::{AtomicI32, Ordering},
    time::Duration,
};

use crate::config::NetworkConstants;

static NEVER_EXPIRES: AtomicI32 = AtomicI32::new(0);

#[derive(Clone)]
pub struct WorkTicket<'a> {
    ticket: &'a AtomicI32,
    ticket_copy: i32,
}

impl<'a> WorkTicket<'a> {
    pub fn never_expires() -> Self {
        Self::new(&NEVER_EXPIRES)
    }

    pub fn new(ticket: &'a AtomicI32) -> Self {
        Self {
            ticket,
            ticket_copy: ticket.load(Ordering::SeqCst),
        }
    }

    pub fn expired(&self) -> bool {
        self.ticket_copy != self.ticket.load(Ordering::SeqCst)
    }
}

pub struct WorkPool {
    network_constants: NetworkConstants,
    max_threads: u32,
    pow_rate_limiter: Duration,
    ticket: AtomicI32,
}

impl WorkPool {
    pub fn new(
        network_constants: NetworkConstants,
        max_threads: u32,
        pow_rate_limiter: Duration,
    ) -> Self {
        Self {
            network_constants,
            max_threads,
            pow_rate_limiter,
            ticket: AtomicI32::new(0),
        }
    }

    pub fn create_work_ticket(&'_ self) -> WorkTicket<'_> {
        WorkTicket::new(&self.ticket)
    }

    pub fn expire_tickets(&self) {
        self.ticket.fetch_add(1, Ordering::SeqCst);
    }
}
