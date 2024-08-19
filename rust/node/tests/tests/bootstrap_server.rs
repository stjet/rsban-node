use super::helpers::{assert_timely_eq, make_fake_channel, setup_chains, System};
use crate::tests::helpers::assert_always_eq;
use rsnano_core::{BlockEnum, DEV_GENESIS_KEY};
use rsnano_messages::{
    AscPullAck, AscPullAckType, AscPullReq, AscPullReqType, BlocksReqPayload, HashType, Message,
};
use rsnano_node::{bootstrap::BootstrapServer, node::Node};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

#[test]
fn serve_account_blocks() {
    let mut system = System::new();
    let node = system.make_node();

    let responses = ResponseHelper::new();
    responses.connect(&node);

    let mut chains = setup_chains(&node, 1, 128, &DEV_GENESIS_KEY, true);
    let (first_account, first_blocks) = chains.pop().unwrap();

    // Request blocks from account root
    let request = Message::AscPullReq(AscPullReq {
        id: 7,
        req_type: AscPullReqType::Blocks(BlocksReqPayload {
            start_type: HashType::Account,
            start: first_account.into(),
            count: BootstrapServer::MAX_BLOCKS as u8,
        }),
    });

    let channel = make_fake_channel(&node);
    node.inbound_message_queue.put(request, channel);

    assert_timely_eq(Duration::from_secs(5), || responses.len(), 1);

    let response = responses.get().pop().unwrap();
    // Ensure we got response exactly for what we asked for
    assert_eq!(response.id, 7);
    let AscPullAckType::Blocks(response_payload) = response.pull_type else {
        panic!("wrong ack type")
    };

    assert_eq!(response_payload.blocks().len(), 128);
    assert!(compare_blocks(response_payload.blocks(), &first_blocks));

    // Ensure we don't get any unexpected responses
    assert_always_eq(Duration::from_secs(1), || responses.len(), 1);
}

#[test]
fn serve_hash() {
    let mut system = System::new();
    let node = system.make_node();

    let responses = ResponseHelper::new();
    responses.connect(&node);

    let mut chains = setup_chains(&node, 1, 256, &DEV_GENESIS_KEY, true);
    let (account, blocks) = chains.pop().unwrap();

    // Skip a few blocks to request hash in the middle of the chain
    let blocks = &blocks[9..];

    // Request blocks from the middle of the chain
    let request = Message::AscPullReq(AscPullReq {
        id: 7,
        req_type: AscPullReqType::Blocks(BlocksReqPayload {
            start_type: HashType::Block,
            start: blocks[0].hash().into(),
            count: BootstrapServer::MAX_BLOCKS as u8,
        }),
    });

    let channel = make_fake_channel(&node);
    node.inbound_message_queue.put(request, channel);

    assert_timely_eq(Duration::from_secs(5), || responses.len(), 1);

    let response = responses.get().pop().unwrap();
    // Ensure we got response exactly for what we asked for
    assert_eq!(response.id, 7);
    let AscPullAckType::Blocks(response_payload) = response.pull_type else {
        panic!("wrong ack type")
    };

    assert_eq!(response_payload.blocks().len(), 128);
    assert!(compare_blocks(response_payload.blocks(), blocks));

    // Ensure we don't get any unexpected responses
    assert_always_eq(Duration::from_secs(1), || responses.len(), 1);
}

struct ResponseHelper {
    responses: Arc<Mutex<Vec<AscPullAck>>>,
}

impl ResponseHelper {
    fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn len(&self) -> usize {
        self.responses.lock().unwrap().len()
    }

    fn get(&self) -> Vec<AscPullAck> {
        self.responses.lock().unwrap().clone()
    }

    fn connect(&self, node: &Node) {
        let responses = self.responses.clone();
        node.bootstrap_server
            .set_response_callback(Box::new(move |response, _channel| {
                responses.lock().unwrap().push(response.clone());
            }));
    }
}

/// Checks if both lists contain the same blocks, with `blocks_b`
fn compare_blocks(blocks_a: &[BlockEnum], blocks_b: &[BlockEnum]) -> bool {
    assert!(blocks_a.len() <= blocks_b.len());
    blocks_a.iter().zip(blocks_b.iter()).all(|(a, b)| a == b)
}
