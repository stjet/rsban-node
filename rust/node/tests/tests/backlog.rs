use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use rsnano_core::DEV_GENESIS_KEY;
use test_helpers::{assert_timely, setup_independent_blocks, System};

/*
 * Ensures all not confirmed accounts get activated by backlog scan periodically
 */
#[test]
fn backlog_population() {
    let activated = Arc::new(Mutex::new(HashSet::new()));
    let activated2 = activated.clone();
    let mut system = System::new();
    let node = system.make_node();

    node.backlog_population
        .set_activate_callback(Box::new(move |_tx, account| {
            activated2.lock().unwrap().insert(*account);
        }));

    let blocks = setup_independent_blocks(&node, 256, &DEV_GENESIS_KEY);

    // Checks if `activated` set contains all accounts we previously set up
    assert_timely(Duration::from_secs(5), || {
        let guard = activated.lock().unwrap();
        blocks.iter().all(|b| guard.contains(&b.account()))
    });

    // Clear activated set to ensure we activate those accounts more than once
    activated.lock().unwrap().clear();

    assert_timely(Duration::from_secs(5), || {
        let guard = activated.lock().unwrap();
        blocks.iter().all(|b| guard.contains(&b.account()))
    });
}
