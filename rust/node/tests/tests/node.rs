use std::time::Duration;

use crate::tests::helpers::System;

#[test]
#[ignore = "todo"]
fn local_block_broadcast() {
    let mut system = System::new();

    let mut node_config = System::default_config();
    node_config.priority_scheduler_enabled = false;
    node_config.hinted_scheduler.enabled = false;
    node_config.optimistic_scheduler.enabled = false;
    node_config.local_block_broadcaster.rebroadcast_interval = Duration::from_secs(1);

    let _node1 = system.build_node().config(node_config).finish();

    assert_eq!(1 + 1, 2);
}
