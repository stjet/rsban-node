use rsnano_core::{Account, Amount, Block, KeyPair, Root, StateBlock, Vote, DEV_GENESIS_KEY};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
use rsnano_messages::{ConfirmAck, Keepalive, Message, Publish};
use rsnano_network::{ChannelMode, DropPolicy, TrafficType};
use rsnano_node::{
    bootstrap::BootstrapInitiatorExt,
    stats::{DetailType, Direction, StatType},
    wallets::WalletsExt,
};
use std::{ops::Deref, sync::Arc, time::Duration};
use test_helpers::{
    assert_timely, assert_timely_eq, assert_timely_msg, establish_tcp, make_fake_channel,
    start_election, System,
};

#[test]
fn last_contacted() {
    let mut system = System::new();

    let node0 = system.make_node();

    let mut node1_config = System::default_config();
    node1_config.tcp_incoming_connections_max = 0; // Prevent ephemeral node1->node0 channel repacement with incoming connection
    let node1 = system
        .build_node()
        .config(node1_config)
        .disconnected()
        .finish();

    let channel1 = establish_tcp(&node1, &node0);
    assert_timely_eq(
        Duration::from_secs(3),
        || {
            node0
                .network_info
                .read()
                .unwrap()
                .count_by_mode(ChannelMode::Realtime)
        },
        1,
    );

    // channel0 is the other side of channel1, same connection different endpoint
    let channel0 = node0
        .network_info
        .read()
        .unwrap()
        .find_node_id(&node1.node_id.public_key())
        .unwrap()
        .clone();

    // check that the endpoints are part of the same connection
    assert_eq!(channel0.local_addr(), channel1.peer_addr());
    assert_eq!(channel1.local_addr(), channel0.peer_addr());

    // capture the state before and ensure the clock ticks at least once
    let timestamp_before_keepalive = channel0.last_activity();
    let keepalive_count =
        node0
            .stats
            .count(StatType::Message, DetailType::Keepalive, Direction::In);
    assert_timely_msg(
        Duration::from_secs(3),
        || node0.steady_clock.now() > timestamp_before_keepalive,
        "clock did not advance",
    );

    // send 3 keepalives
    // we need an extra keepalive to handle the race condition between the timestamp set and the counter increment
    // and we need one more keepalive to handle the possibility that there is a keepalive already in flight when we start the crucial part of the test
    // it is possible that there could be multiple keepalives in flight but we assume here that there will be no more than one in flight for the purposes of this test
    let keepalive = Message::Keepalive(Keepalive::default());
    let mut publisher = node0.message_publisher.lock().unwrap();
    publisher.try_send(
        channel1.channel_id(),
        &keepalive,
        DropPolicy::ShouldNotDrop,
        TrafficType::Generic,
    );
    publisher.try_send(
        channel1.channel_id(),
        &keepalive,
        DropPolicy::ShouldNotDrop,
        TrafficType::Generic,
    );
    publisher.try_send(
        channel1.channel_id(),
        &keepalive,
        DropPolicy::ShouldNotDrop,
        TrafficType::Generic,
    );

    assert_timely_msg(
        Duration::from_secs(3),
        || {
            node0
                .stats
                .count(StatType::Message, DetailType::Keepalive, Direction::In)
                >= keepalive_count + 3
        },
        "keepalive count",
    );
    assert_eq!(
        node0
            .network_info
            .read()
            .unwrap()
            .count_by_mode(ChannelMode::Realtime),
        1
    );
    let timestamp_after_keepalive = channel0.last_activity();
    assert!(timestamp_after_keepalive > timestamp_before_keepalive);
}

#[test]
fn send_discarded_publish() {
    let mut system = System::new();
    let node1 = system.make_node();
    let node2 = system.make_node();

    let block = Block::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        2.into(),
        3.into(),
        Amount::MAX,
        4.into(),
        &DEV_GENESIS_KEY,
        node1.work_generate_dev(Root::from(2)),
    ));

    node1.message_publisher.lock().unwrap().flood(
        &Message::Publish(Publish::new_forward(block)),
        DropPolicy::ShouldNotDrop,
        1.0,
    );

    assert_eq!(node1.latest(&DEV_GENESIS_ACCOUNT), *DEV_GENESIS_HASH);
    assert_eq!(node2.latest(&DEV_GENESIS_ACCOUNT), *DEV_GENESIS_HASH);
    assert_timely_msg(
        Duration::from_secs(10),
        || {
            node2
                .stats
                .count(StatType::Message, DetailType::Publish, Direction::In)
                != 0
        },
        "no publish received",
    );
    assert_eq!(node1.latest(&DEV_GENESIS_ACCOUNT), *DEV_GENESIS_HASH);
    assert_eq!(node2.latest(&DEV_GENESIS_ACCOUNT), *DEV_GENESIS_HASH);
}

#[test]
fn receivable_processor_confirm_insufficient_pos() {
    let mut system = System::new();
    let node1 = system.make_node();
    let send1 = Block::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::raw(1),
        Account::zero().into(),
        &DEV_GENESIS_KEY,
        node1.work_generate_dev(*DEV_GENESIS_HASH),
    ));

    node1.process(send1.clone()).unwrap();
    let election = start_election(&node1, &send1.hash());

    let key1 = KeyPair::new();
    let vote = Arc::new(Vote::new_final(&key1, vec![send1.hash()]));
    let channel = make_fake_channel(&node1);
    let con1 = Message::ConfirmAck(ConfirmAck::new_with_rebroadcasted_vote(
        vote.deref().clone(),
    ));
    assert_eq!(1, election.vote_count());

    node1.inbound_message_queue.put(con1, channel.info.clone());

    assert_timely_eq(Duration::from_secs(5), || election.vote_count(), 2);
}

#[test]
fn receivable_processor_confirm_sufficient_pos() {
    let mut system = System::new();
    let node1 = system.make_node();
    let send1 = Block::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::raw(1),
        Account::zero().into(),
        &DEV_GENESIS_KEY,
        node1.work_generate_dev(*DEV_GENESIS_HASH),
    ));

    node1.process(send1.clone()).unwrap();
    let election = start_election(&node1, &send1.hash());

    let vote = Arc::new(Vote::new_final(&DEV_GENESIS_KEY, vec![send1.hash()]));
    let channel = make_fake_channel(&node1);
    let con1 = Message::ConfirmAck(ConfirmAck::new_with_rebroadcasted_vote(
        vote.deref().clone(),
    ));
    assert_eq!(1, election.vote_count());

    node1.inbound_message_queue.put(con1, channel.info.clone());

    assert_timely_eq(Duration::from_secs(5), || election.vote_count(), 2);
}

#[test]
fn multi_keepalive() {
    let mut system = System::new();
    let node1 = system.make_node();
    let _node2 = system.make_node();
    assert_timely(Duration::from_secs(10), || {
        node1
            .stats
            .count(StatType::Message, DetailType::Keepalive, Direction::In)
            > 0
    });
}

#[test]
fn send_valid_confirm_ack() {
    let mut system = System::new();
    let node1 = system.make_node();
    let node2 = system.make_node();
    let key2 = KeyPair::new();
    node1.insert_into_wallet(&DEV_GENESIS_KEY);
    node2.insert_into_wallet(&key2);
    let block2 = Block::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::raw(50),
        key2.public_key().as_account().into(),
        &DEV_GENESIS_KEY,
        node1.work_generate_dev(*DEV_GENESIS_HASH),
    ));
    node1.process_active(block2);
    // Keep polling until latest block changes
    assert_timely(Duration::from_secs(10), || {
        node2.latest(&DEV_GENESIS_ACCOUNT) != *DEV_GENESIS_HASH
    });
    // Make sure the balance has decreased after processing the block.
    assert_eq!(node2.balance(&DEV_GENESIS_ACCOUNT), Amount::raw(50));
}

#[test]
fn send_valid_publish() {
    let mut system = System::new();
    let node1 = system.make_node();
    let node2 = system.make_node();
    node1.bootstrap_initiator.stop();
    node2.bootstrap_initiator.stop();
    node1.insert_into_wallet(&DEV_GENESIS_KEY);
    let key2 = KeyPair::new();
    node2.insert_into_wallet(&key2);
    let block2 = Block::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::raw(50),
        key2.public_key().as_account().into(),
        &DEV_GENESIS_KEY,
        node1.work_generate_dev(*DEV_GENESIS_HASH),
    ));
    let hash2 = block2.hash();
    let latest2 = node2.latest(&DEV_GENESIS_ACCOUNT);
    node2.process_active(block2);
    assert_timely(Duration::from_secs(10), || {
        node1
            .stats
            .count(StatType::Message, DetailType::Publish, Direction::In)
            > 0
    });
    assert_ne!(hash2, latest2);
    assert_timely(Duration::from_secs(10), || {
        node2.latest(&DEV_GENESIS_ACCOUNT) != latest2
    });
    assert_eq!(node2.balance(&DEV_GENESIS_ACCOUNT), Amount::raw(50));
}

#[test]
fn send_with_receive() {
    let mut system = System::new();
    let node1 = system.make_node();
    let node2 = system.make_node();
    let key2 = KeyPair::new();
    node1.insert_into_wallet(&DEV_GENESIS_KEY);
    let block1 = Block::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - node1.config.receive_minimum,
        key2.public_key().as_account().into(),
        &DEV_GENESIS_KEY,
        node1.work_generate_dev(*DEV_GENESIS_HASH),
    ));

    node1.process_active(block1.clone());
    assert_timely(Duration::from_secs(5), || {
        node1.block_exists(&block1.hash())
    });
    node2.process_active(block1.clone());
    assert_timely(Duration::from_secs(5), || {
        node2.block_exists(&block1.hash())
    });
    node2.insert_into_wallet(&key2);
    assert_timely(Duration::from_secs(10), || {
        node1.balance(&key2.public_key().as_account()) == node1.config.receive_minimum
            && node2.balance(&key2.public_key().as_account()) == node1.config.receive_minimum
    });
}

#[test]
fn receive_weight_change() {
    let mut system = System::new();
    let node1 = system.make_node();
    let node2 = system.make_node();
    let key2 = KeyPair::new();
    node1.insert_into_wallet(&DEV_GENESIS_KEY);
    node2.insert_into_wallet(&key2);
    node2
        .wallets
        .set_representative(node2.wallets.wallet_ids()[0], key2.public_key(), false)
        .unwrap();
    node1
        .wallets
        .send_action2(
            &node1.wallets.wallet_ids()[0],
            *DEV_GENESIS_ACCOUNT,
            key2.public_key().as_account(),
            node1.config.receive_minimum,
            0,
            true,
            None,
        )
        .unwrap();
    assert_timely(Duration::from_secs(10), || {
        node1
            .ledger
            .weight_exact(&node1.ledger.read_txn(), key2.public_key())
            == node1.config.receive_minimum
            && node2
                .ledger
                .weight_exact(&node2.ledger.read_txn(), key2.public_key())
                == node1.config.receive_minimum
    });
}