use std::sync::Arc;

use crate::tests::helpers::make_fake_channel;

use super::helpers::{start_election, System};
use rsnano_core::{
    Amount, BlockEnum, KeyPair, Signature, StateBlock, Vote, VoteCode, VoteSource, DEV_GENESIS_KEY,
};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH};

#[test]
fn check_signature() {
    let mut system = System::new();
    let mut config = System::default_config();
    config.online_weight_minimum = Amount::MAX;
    let node = system.build_node().config(config).finish();
    let key1 = KeyPair::new();
    let send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_ACCOUNT,
        Amount::MAX - Amount::raw(100),
        key1.public_key().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev((*DEV_GENESIS_HASH).into()),
    ));
    node.process(send1.clone()).unwrap();
    let election1 = start_election(&node, &send1.hash());
    assert_eq!(1, election1.mutex.lock().unwrap().last_votes.len());
    let mut vote1 = Vote::new(
        *DEV_GENESIS_ACCOUNT,
        &DEV_GENESIS_KEY.private_key(),
        Vote::TIMESTAMP_MIN,
        0,
        vec![send1.hash()],
    );
    let good_signature = vote1.signature;
    vote1.signature = Signature::new();
    let channel = make_fake_channel(&node);
    assert_eq!(
        VoteCode::Invalid,
        node.vote_processor.vote_blocking(
            &Arc::new(vote1.clone()),
            &Some(channel.clone()),
            VoteSource::Live
        )
    );

    vote1.signature = good_signature;
    assert_eq!(
        VoteCode::Vote,
        node.vote_processor.vote_blocking(
            &Arc::new(vote1.clone()),
            &Some(channel.clone()),
            VoteSource::Live
        )
    );
    assert_eq!(
        VoteCode::Replay,
        node.vote_processor.vote_blocking(
            &Arc::new(vote1.clone()),
            &Some(channel.clone()),
            VoteSource::Live
        )
    );
}
