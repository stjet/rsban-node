//use rsnano_core::{
//    work::WorkPool, Amount, BlockEnum, BlockHash, KeyPair, StateBlock, Vote, DEV_GENESIS_KEY,
//};
//use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH};
//use rsnano_node::config::FrontiersConfirmationMode;
//
//use super::helpers::System;

/// What this test is doing:
/// Create 20 representatives with minimum principal weight each
/// Create a send block on the genesis account (the last send block)
/// Create 20 forks of the last send block using genesis as representative (no votes produced)
/// Check that only 10 blocks remain in the election (due to max 10 forks per election object limit)
/// Create 20 more forks of the last send block using the new reps as representatives and produce votes for them
///     (9 votes from this batch should survive and replace existing blocks in the election, why not 10?)
/// Then send winning block and it should replace one of the existing blocks
#[test]
fn fork_replacement_tally() {
    //let mut system = System::new();
    //let mut node_config = System::default_config();
    //node_config.frontiers_confirmation = FrontiersConfirmationMode::Disabled;
    //let node1 = system.build_node().config(node_config).finish();

    //const REPS_COUNT: usize = 20;
    //const MAX_BLOCKS: usize = 10;

    //let keys: Vec<_> = std::iter::repeat_with(|| KeyPair::new())
    //    .take(REPS_COUNT)
    //    .collect();
    //let mut latest = *DEV_GENESIS_HASH;
    //let mut balance = Amount::MAX;
    //let amount = node1.online_reps.lock().unwrap().minimum_principal_weight();

    //// Create 20 representatives & confirm blocks
    //for i in 0..REPS_COUNT {
    //    balance = balance - (amount + Amount::raw(i as u128));
    //    let send = BlockEnum::State(StateBlock::new(
    //        *DEV_GENESIS_ACCOUNT,
    //        latest,
    //        *DEV_GENESIS_ACCOUNT,
    //        balance,
    //        keys[i].public_key().into(),
    //        &DEV_GENESIS_KEY,
    //        system.work.generate_dev2(latest.into()).unwrap(),
    //    ));
    //    node1.process_active(send.clone());
    //    latest = send.hash();
    //    let open = BlockEnum::State(StateBlock::new(
    //        keys[i].public_key(),
    //        BlockHash::zero(),
    //        keys[i].public_key(),
    //        amount + Amount::raw(i as u128),
    //        send.hash().into(),
    //        &keys[i],
    //        system
    //            .work
    //            .generate_dev2(keys[i].public_key().into())
    //            .unwrap(),
    //    ));
    //    node1.process_active(open.clone());
    //    // Confirmation
    //    let vote = Vote::new_final(&DEV_GENESIS_KEY, vec![send.hash(), open.hash()]);
    //    node1
    //        .vote_processor_queue
    //        .vote(Arc::new(vote), channel, source)
    //}
    // TODO port remainig part
}
