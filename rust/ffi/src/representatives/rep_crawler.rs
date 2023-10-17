use rsnano_core::BlockHash;
use rsnano_node::representatives::RepCrawler;
use std::{
    ffi::{c_char, CStr},
    sync::Arc,
};

use crate::{transport::ChannelHandle, utils::ContainerInfoComponentHandle, voting::VoteHandle};

use super::representative::RepresentativeHandle;

pub struct RepCrawlerHandle(pub Arc<RepCrawler>);

#[no_mangle]
pub unsafe extern "C" fn rsn_rep_crawler_create() -> *mut RepCrawlerHandle {
    Box::into_raw(Box::new(RepCrawlerHandle(Arc::new(RepCrawler::new()))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rep_crawler_destroy(handle: *mut RepCrawlerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rep_crawler_add(
    handle: *mut RepCrawlerHandle,
    rep: *mut RepresentativeHandle,
) {
    (*handle).0.add_rep((*rep).0.clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rep_crawler_remove(handle: *mut RepCrawlerHandle, hash: *const u8) {
    (*handle).0.remove(&BlockHash::from_ptr(hash));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rep_crawler_active_contains(
    handle: *mut RepCrawlerHandle,
    hash: *const u8,
) -> bool {
    (*handle).0.active_contains(&BlockHash::from_ptr(hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rep_crawler_active_insert(
    handle: *mut RepCrawlerHandle,
    hash: *const u8,
) {
    (*handle).0.insert_active(BlockHash::from_ptr(hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rep_crawler_response_insert(
    handle: *mut RepCrawlerHandle,
    channel: *mut ChannelHandle,
    vote: &VoteHandle,
) {
    (*handle)
        .0
        .insert_response((*channel).0.clone(), vote.0.clone());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rep_crawler_collect_container_info(
    handle: *const RepCrawlerHandle,
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let container_info = (*handle)
        .0
        .collect_container_info(CStr::from_ptr(name).to_str().unwrap().to_owned());
    Box::into_raw(Box::new(ContainerInfoComponentHandle(container_info)))
}
