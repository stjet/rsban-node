use async_trait::async_trait;
use rsnano_core::utils::StreamAdapter;
use std::sync::{Arc, Mutex};

use crate::{
    config::NetworkConstants,
    messages::{Message, MessageHeader, ProtocolInfo},
    transport::validate_header,
    utils::BlockUniquer,
    voting::VoteUniquer,
};

use super::{MessageDeserializerImpl, NetworkFilter, ParseStatus, MAX_MESSAGE_SIZE};

#[async_trait]
pub trait AsyncBufferReader {
    async fn read(&self, buffer: Arc<Mutex<Vec<u8>>>, count: usize) -> anyhow::Result<()>;
}

pub struct AsyncMessageDeserializer<T: AsyncBufferReader + Send> {
    deserializer_impl: MessageDeserializerImpl,
    protocol_info: ProtocolInfo,
    read_buffer: Arc<Mutex<Vec<u8>>>,
    buffer_reader: T,
}

impl<T: AsyncBufferReader + Send> AsyncMessageDeserializer<T> {
    pub fn new(
        network_constants: NetworkConstants,
        network_filter: Arc<NetworkFilter>,
        block_uniquer: Arc<BlockUniquer>,
        vote_uniquer: Arc<VoteUniquer>,
        buffer_reader: T,
    ) -> Self {
        Self {
            deserializer_impl: MessageDeserializerImpl::new(
                network_constants.clone(),
                network_filter,
                block_uniquer,
                vote_uniquer,
            ),
            protocol_info: network_constants.protocol_info(),
            read_buffer: Arc::new(Mutex::new(vec![0; MAX_MESSAGE_SIZE])),
            buffer_reader,
        }
    }

    pub async fn read(&self) -> Result<Box<dyn Message>, ParseStatus> {
        self.buffer_reader
            .read(
                Arc::clone(&self.read_buffer),
                MessageHeader::SERIALIZED_SIZE,
            )
            .await
            .map_err(|_| ParseStatus::None)?;

        self.received_header().await
    }

    async fn received_header(&self) -> Result<Box<dyn Message>, ParseStatus> {
        let header = {
            let buffer = self.read_buffer.lock().unwrap();
            let mut stream = StreamAdapter::new(&buffer[..MessageHeader::SERIALIZED_SIZE]);
            MessageHeader::deserialize(&mut stream).map_err(|_| ParseStatus::InvalidHeader)?
        };

        validate_header(&header, &self.protocol_info)?;
        let payload_size = header.payload_length();
        if payload_size == 0 {
            // Payload size will be 0 for `bulk_push` & `telemetry_req` message type
            self.received_message(header, 0)
        } else {
            self.buffer_reader
                .read(Arc::clone(&self.read_buffer), payload_size)
                .await
                .map_err(|_| ParseStatus::None)?; // TODO return correct error code
            self.received_message(header, payload_size)
        }
    }

    fn received_message(
        &self,
        header: MessageHeader,
        payload_size: usize,
    ) -> Result<Box<dyn Message>, ParseStatus> {
        let buffer = self.read_buffer.lock().unwrap();
        let result = self
            .deserializer_impl
            .deserialize(header, &buffer[..payload_size]);
        result
    }
}
