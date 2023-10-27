use async_trait::async_trait;
use rsnano_core::utils::StreamAdapter;
use std::sync::{Arc, Mutex};

use crate::{
    config::NetworkConstants,
    messages::{Message, MessageHeader, ProtocolInfo},
    transport::validate_header,
    utils::{BlockUniquer, ErrorCode},
    voting::VoteUniquer,
};

use super::{MessageDeserializerImpl, NetworkFilter, ParseStatus, MAX_MESSAGE_SIZE};

pub type ReadQuery =
    Box<dyn Fn(Arc<Mutex<Vec<u8>>>, usize, Box<dyn FnOnce(ErrorCode, usize) + Send>) + Send + Sync>;

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
            MessageHeader::from_stream(&mut stream).map_err(|_| ParseStatus::InvalidHeader)?
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
        self.deserializer_impl
            .deserialize(header, &buffer[..payload_size])
    }
}

// TODO delete
pub struct MessageDeserializer {
    deserializer_impl: MessageDeserializerImpl,
    network_constants: NetworkConstants,
    read_buffer: Arc<Mutex<Vec<u8>>>,
    status: Mutex<ParseStatus>,
    read_op: ReadQuery,
}

impl MessageDeserializer {
    pub fn new(
        network_constants: NetworkConstants,
        network_filter: Arc<NetworkFilter>,
        block_uniquer: Arc<BlockUniquer>,
        vote_uniquer: Arc<VoteUniquer>,
        read_op: ReadQuery,
    ) -> Self {
        Self {
            deserializer_impl: MessageDeserializerImpl::new(
                network_constants.clone(),
                network_filter,
                block_uniquer,
                vote_uniquer,
            ),
            network_constants,
            status: Mutex::new(ParseStatus::None),
            read_buffer: Arc::new(Mutex::new(vec![0; MAX_MESSAGE_SIZE])),
            read_op,
        }
    }

    pub fn status(&self) -> ParseStatus {
        *self.status.lock().unwrap()
    }

    fn set_status(&self, status: ParseStatus) {
        let mut guard = self.status.lock().unwrap();
        *guard = status;
    }

    fn received_message(&self, header: MessageHeader, payload_size: usize, callback: CallbackType) {
        assert!(payload_size <= MAX_MESSAGE_SIZE);
        let result = {
            let buffer = self.read_buffer.lock().unwrap();
            self.deserializer_impl
                .deserialize(header, &buffer[..payload_size])
        };

        match result {
            Ok(message) => {
                self.set_status(ParseStatus::Success);
                callback(ErrorCode::new(), Some(message));
            }
            Err(status) => {
                self.set_status(status);
                callback(ErrorCode::new(), None);
            }
        }
    }
}

pub type CallbackType = Box<dyn FnOnce(ErrorCode, Option<Box<dyn Message>>) + Send>;

pub trait MessageDeserializerExt {
    /// Asynchronously read next message from channel_read_fn.
    /// If an irrecoverable error is encountered callback will be called with an error code set and `None` message.
    /// If a 'soft' error is encountered (eg. duplicate block publish) error won't be set but message will be `None`. In that case, `status` field will be set to code indicating reason for failure.
    /// If message is received successfully, error code won't be set and message will be non-null. `status` field will be set to `success`.
    /// Should not be called until the previous invocation finishes and calls the callback.
    fn read(&self, callback: CallbackType);

    /// Deserializes message using data in `read_buffer`.
    /// # Return
    /// If successful returns non-null message, otherwise sets `status` to error appropriate code and returns `None`
    fn received_header(&self, callback: CallbackType);
}

impl MessageDeserializerExt for Arc<MessageDeserializer> {
    fn read(&self, callback: CallbackType) {
        self.set_status(ParseStatus::None);

        let self_clone = Arc::clone(self);
        (self.read_op)(
            Arc::clone(&self.read_buffer),
            MessageHeader::SERIALIZED_SIZE,
            Box::new(move |ec, size| {
                if ec.is_err() {
                    callback(ec, None);
                    return;
                }
                if size != MessageHeader::SERIALIZED_SIZE {
                    callback(ErrorCode::fault(), None);
                    return;
                }

                self_clone.received_header(callback);
            }),
        );
    }

    fn received_header(&self, callback: CallbackType) {
        let header = {
            let buffer = self.read_buffer.lock().unwrap();
            let mut stream = StreamAdapter::new(&buffer[..MessageHeader::SERIALIZED_SIZE]);
            MessageHeader::from_stream(&mut stream)
        };

        let header = match header {
            Ok(header) => header,
            Err(_) => {
                callback(ErrorCode::fault(), None);
                return;
            }
        };

        if let Err(status) = validate_header(&header, &self.network_constants.protocol_info()) {
            self.set_status(status);
            callback(ErrorCode::fault(), None);
            return;
        }

        let payload_size = header.payload_length();

        if payload_size == 0 {
            // Payload size will be 0 for `bulk_push` & `telemetry_req` message type
            self.received_message(header, 0, callback);
        } else {
            let self_clone = Arc::clone(self);
            (self.read_op)(
                Arc::clone(&self.read_buffer),
                payload_size,
                Box::new(move |ec, size| {
                    if ec.is_err() {
                        callback(ec, None);
                        return;
                    }
                    if size != payload_size {
                        callback(ErrorCode::fault(), None);
                        return;
                    }
                    self_clone.received_message(header, size, callback);
                }),
            );
        }
    }
}
