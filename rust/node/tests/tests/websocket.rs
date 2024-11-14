use core::panic;
use futures_util::{SinkExt, StreamExt};
use rsnano_core::{Amount, BlockEnum, KeyPair, Networks, SendBlock, StateBlock, DEV_GENESIS_KEY};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
use rsnano_messages::{Message, Publish};
use rsnano_node::{
    config::{NetworkConstants, NodeConfig},
    websocket::{BlockConfirmed, OutgoingMessageEnvelope, Topic, WebsocketConfig},
    Node,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use test_helpers::{assert_timely, get_available_port, make_fake_channel, System};
use tokio::{net::TcpStream, task::spawn_blocking, time::timeout};
use tokio_tungstenite::{connect_async, tungstenite, MaybeTlsStream, WebSocketStream};

/// Tests getting notification of a started election
#[test]
fn started_election() {
    let mut system = System::new();
    let node1 = create_node_with_websocket(&mut system);
    let channel1 = make_fake_channel(&node1);
    node1.runtime.block_on(async {
        let mut ws_stream = connect_websocket(&node1).await;
        ws_stream
            .send(tungstenite::Message::Text(
                r#"{"action": "subscribe", "topic": "started_election", "ack": true}"#.to_string(),
            ))
            .await
            .unwrap();

        //await ack
        ws_stream.next().await.unwrap().unwrap();

        assert_eq!(
            1,
            node1
                .websocket
                .as_ref()
                .unwrap()
                .subscriber_count(Topic::StartedElection)
        );

        // Create election, causing a websocket message to be emitted
        let key1 = KeyPair::new();
        let send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            Amount::zero(),
            key1.account().into(),
            &DEV_GENESIS_KEY,
            node1.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));
        let publish1 = Message::Publish(Publish::new_forward(send1.clone()));
        node1
            .inbound_message_queue
            .put(publish1, channel1.info.clone());
        assert_timely(Duration::from_secs(1), || {
            node1.active.election(&send1.qualified_root()).is_some()
        });

        let Ok(response) = timeout(Duration::from_secs(5), ws_stream.next()).await else {
            panic!("timeout");
        };
        let response = response.unwrap().unwrap();
        let response_msg: OutgoingMessageEnvelope =
            serde_json::from_str(response.to_text().unwrap()).unwrap();
        assert_eq!(response_msg.topic, Some(Topic::StartedElection));
    });
}

// Tests getting notification of an erased election
#[test]
fn stopped_election() {
    let mut system = System::new();
    let node1 = create_node_with_websocket(&mut system);
    let channel1 = make_fake_channel(&node1);
    node1.runtime.block_on(async {
        let mut ws_stream = connect_websocket(&node1).await;
        ws_stream
            .send(tungstenite::Message::Text(
                r#"{"action": "subscribe", "topic": "stopped_election", "ack": true}"#.to_string(),
            ))
            .await
            .unwrap();

        //await ack
        ws_stream.next().await.unwrap().unwrap();

        assert_eq!(
            1,
            node1
                .websocket
                .as_ref()
                .unwrap()
                .subscriber_count(Topic::StoppedElection)
        );

        // Create election, then erase it, causing a websocket message to be emitted
        let key1 = KeyPair::new();
        let send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            Amount::zero(),
            key1.account().into(),
            &DEV_GENESIS_KEY,
            node1.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));
        let publish1 = Message::Publish(Publish::new_forward(send1.clone()));
        node1
            .inbound_message_queue
            .put(publish1, channel1.info.clone());
        assert_timely(Duration::from_secs(1), || {
            node1.active.election(&send1.qualified_root()).is_some()
        });
        let active = node1.active.clone();
        spawn_blocking(move || active.erase(&send1.qualified_root()))
            .await
            .unwrap();

        let Ok(response) = timeout(Duration::from_secs(5), ws_stream.next()).await else {
            panic!("timeout");
        };
        let response = response.unwrap().unwrap();
        let response_msg: OutgoingMessageEnvelope =
            serde_json::from_str(response.to_text().unwrap()).unwrap();
        assert_eq!(response_msg.topic, Some(Topic::StoppedElection));
    });
}

#[test]
// Tests clients subscribing multiple times or unsubscribing without a subscription
fn subscription_edge() {
    let mut system = System::new();
    let node1 = create_node_with_websocket(&mut system);
    let websocket = node1.websocket.as_ref().unwrap();
    assert_eq!(websocket.subscriber_count(Topic::Confirmation), 0);

    node1.runtime.block_on(async {
        let mut ws_stream = connect_websocket(&node1).await;
        ws_stream
            .send(tungstenite::Message::Text(
                r#"{"action": "subscribe", "topic": "confirmation", "ack": true}"#.to_string(),
            ))
            .await
            .unwrap();
        //await ack
        ws_stream.next().await.unwrap().unwrap();
        assert_eq!(websocket.subscriber_count(Topic::Confirmation), 1);
        ws_stream
            .send(tungstenite::Message::Text(
                r#"{"action": "subscribe", "topic": "confirmation", "ack": true}"#.to_string(),
            ))
            .await
            .unwrap();
        //await ack
        ws_stream.next().await.unwrap().unwrap();
        assert_eq!(websocket.subscriber_count(Topic::Confirmation), 1);
        ws_stream
            .send(tungstenite::Message::Text(
                r#"{"action": "unsubscribe", "topic": "confirmation", "ack": true}"#.to_string(),
            ))
            .await
            .unwrap();
        //await ack
        ws_stream.next().await.unwrap().unwrap();
        assert_eq!(websocket.subscriber_count(Topic::Confirmation), 0);
        ws_stream
            .send(tungstenite::Message::Text(
                r#"{"action": "unsubscribe", "topic": "confirmation", "ack": true}"#.to_string(),
            ))
            .await
            .unwrap();
        //await ack
        ws_stream.next().await.unwrap().unwrap();
        assert_eq!(websocket.subscriber_count(Topic::Confirmation), 0);
    });
}

#[test]
// Subscribes to block confirmations, confirms a block and then awaits websocket notification
fn confirmation() {
    let mut system = System::new();
    let node1 = create_node_with_websocket(&mut system);
    node1.runtime.block_on(async {
        let mut ws_stream = connect_websocket(&node1).await;
        ws_stream
            .send(tungstenite::Message::Text(
                r#"{"action": "subscribe", "topic": "confirmation", "ack": true}"#.to_string(),
            ))
            .await
            .unwrap();
        //await ack
        ws_stream.next().await.unwrap().unwrap();

        node1.insert_into_wallet(&DEV_GENESIS_KEY);
        let key = KeyPair::new();
        let mut balance = Amount::MAX;
        let send_amount = node1.online_reps.lock().unwrap().quorum_delta() + Amount::raw(1);
        // Quick-confirm a block, legacy blocks should work without filtering
        let mut previous = node1.latest(&DEV_GENESIS_ACCOUNT);
        balance = balance - send_amount;
        let send = BlockEnum::LegacySend(SendBlock::new(
            &previous,
            &key.public_key().as_account(),
            &balance,
            &DEV_GENESIS_KEY.private_key(),
            node1.work_generate_dev(previous.into()),
        ));
        previous = send.hash();
        node1.process_active(send);

        let tungstenite::Message::Text(response) = ws_stream.next().await.unwrap().unwrap() else {
            panic!("not a text message");
        };

        let response_json: OutgoingMessageEnvelope = serde_json::from_str(&response).unwrap();
        assert_eq!(response_json.topic, Some(Topic::Confirmation));

        ws_stream
            .send(tungstenite::Message::Text(
                r#"{"action": "unsubscribe", "topic": "confirmation", "ack": true}"#.to_string(),
            ))
            .await
            .unwrap();
        //await ack
        ws_stream.next().await.unwrap().unwrap();

        // Quick confirm a state block
        balance = balance - send_amount;
        let send = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            previous,
            *DEV_GENESIS_PUB_KEY,
            balance,
            key.public_key().as_account().into(),
            &DEV_GENESIS_KEY,
            node1.work_generate_dev(previous.into()),
        ));
        node1.process_active(send);

        timeout(Duration::from_secs(1), ws_stream.next())
            .await
            .unwrap_err();
    });
}

// Tests the filtering options of block confirmations
#[test]
fn confirmation_options() {
    let mut system = System::new();
    let node1 = create_node_with_websocket(&mut system);
    node1.runtime.block_on(async {
        let mut ws_stream = connect_websocket(&node1).await;
        ws_stream
            .send(tungstenite::Message::Text(
                r#"{"action": "subscribe", "topic": "confirmation", "ack": true, "options": {"confirmation_type": "active_quorum", "accounts": ["xrb_invalid"]}}"#.to_string(),
            ))
            .await
            .unwrap();
        //await ack
        ws_stream.next().await.unwrap().unwrap();
        
        // Confirm a state block for an in-wallet account
        node1.insert_into_wallet(&DEV_GENESIS_KEY);
        let key = KeyPair::new();
        let mut balance = Amount::MAX;
        let send_amount = node1.online_reps.lock().unwrap().quorum_delta() + Amount::raw(1);
        let mut previous = node1.latest(&DEV_GENESIS_ACCOUNT);
        balance = balance - send_amount;
        let send = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            previous,
            *DEV_GENESIS_PUB_KEY,
            balance,
            key.public_key().as_account().into(),
            &DEV_GENESIS_KEY,
            node1.work_generate_dev(previous.into()),
        ));
        previous = send.hash();
        node1.process_active(send);

        timeout(Duration::from_secs(1), ws_stream.next())
            .await
            .unwrap_err();

        ws_stream
            .send(tungstenite::Message::Text(
                r#"{"action": "subscribe", "topic": "confirmation", "ack": true, "options": {"confirmation_type": "active_quorum", "all_local_accounts": true, "include_election_info": true}}"#.to_string(),
            ))
            .await
            .unwrap();
        //await ack
        ws_stream.next().await.unwrap().unwrap();

        // Quick-confirm another block
        balance = balance - send_amount;
        let send = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            previous,
            *DEV_GENESIS_PUB_KEY,
            balance,
            key.public_key().as_account().into(),
            &DEV_GENESIS_KEY,
            node1.work_generate_dev(previous.into()),
        ));
        previous = send.hash();
        node1.process_active(send);

        let tungstenite::Message::Text(response) = ws_stream.next().await.unwrap().unwrap() else {
            panic!("not a text message");
        };

        let response_json: OutgoingMessageEnvelope = serde_json::from_str(&response).unwrap();
        assert_eq!(response_json.topic, Some(Topic::Confirmation));
        let message: BlockConfirmed  = serde_json::from_value(response_json.message.unwrap()).unwrap();
        let election_info = message.election_info.unwrap();
        assert!(election_info.blocks.parse::<i32>().unwrap() >= 1);
		// Make sure tally and time are non-zero.
        assert_ne!(election_info.tally, "0");
        assert_ne!(election_info.time, "0");
        assert!(election_info.votes.is_none());

        ws_stream
            .send(tungstenite::Message::Text(
                r#"{"action": "subscribe", "topic": "confirmation", "ack": true, "options":{"confirmation_type": "active_quorum", "all_local_accounts": true} }"#.to_string(),
            ))
            .await
            .unwrap();
        //await ack
        ws_stream.next().await.unwrap().unwrap();
        
        // Confirm a legacy block
        // When filtering options are enabled, legacy blocks are always filtered
        balance = balance - send_amount;
        let send = BlockEnum::LegacySend(SendBlock::new(&previous, &key.public_key().as_account(), &balance, &DEV_GENESIS_KEY.private_key(), 
                node1.work_generate_dev(previous.into())));
        node1.process_active(send);
        timeout(Duration::from_secs(1), ws_stream.next())
            .await
            .unwrap_err();
    });
}

fn create_node_with_websocket(system: &mut System) -> Arc<Node> {
    let websocket_port = get_available_port();
    let config = NodeConfig {
        websocket_config: WebsocketConfig {
            enabled: true,
            port: websocket_port,
            ..WebsocketConfig::new(&NetworkConstants::default_for(Networks::NanoDevNetwork))
        },
        ..System::default_config()
    };
    let node = system.build_node().config(config).finish();
    node
}

async fn connect_websocket(node: &Node) -> WebSocketStream<MaybeTlsStream<TcpStream>> {
    let (ws_stream, _) = connect_async(format!("ws://[::1]:{}", node.config.websocket_config.port))
        .await
        .expect("Failed to connect");
    ws_stream
}
