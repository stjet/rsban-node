use rsnano_core::{BlockHash, Root};
use rsnano_node::consensus::VoteGenerator;
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

pub struct VoteGeneratorHandle(pub Arc<VoteGenerator>);

impl Deref for VoteGeneratorHandle {
    type Target = Arc<VoteGenerator>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for VoteGeneratorHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_generator_destroy(handle: *mut VoteGeneratorHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_generator_add(
    handle: &VoteGeneratorHandle,
    root: *const u8,
    hash: *const u8,
) {
    handle.add(&Root::from_ptr(root), &BlockHash::from_ptr(hash));
}
