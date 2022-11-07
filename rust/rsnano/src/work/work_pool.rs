use std::{
    sync::atomic::{AtomicI32, Ordering},
    time::Duration,
};

use crate::{
    config::NetworkConstants,
    core::{Root, WorkVersion},
};

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

pub struct WorkPool<'a> {
    network_constants: NetworkConstants,
    max_threads: u32,
    pow_rate_limiter: Duration,
    ticket: AtomicI32,
    opencl: Option<Box<dyn Fn(WorkVersion, Root, u64, WorkTicket<'a>) -> Option<u64>>>,
}

impl<'a> WorkPool<'a> {
    pub fn new<'r>(
        network_constants: NetworkConstants,
        max_threads: u32,
        pow_rate_limiter: Duration,
        opencl: Option<Box<dyn Fn(WorkVersion, Root, u64, WorkTicket<'a>) -> Option<u64>>>,
    ) -> Self {
        Self {
            network_constants,
            max_threads,
            pow_rate_limiter,
            ticket: AtomicI32::new(0),
            opencl,
        }
    }

    pub fn create_work_ticket(&'_ self) -> WorkTicket<'_> {
        WorkTicket::new(&self.ticket)
    }

    pub fn expire_tickets(&self) {
        self.ticket.fetch_add(1, Ordering::SeqCst);
    }

    pub fn call_open_cl(
        &self,
        version: WorkVersion,
        root: Root,
        difficulty: u64,
        ticket: WorkTicket<'a>,
    ) -> Option<u64> {
        match &self.opencl {
            Some(callback) => callback(version, root, difficulty, ticket),
            None => None,
        }
    }

    pub fn has_opencl(&self) -> bool {
        self.opencl.is_some()
    }
}
