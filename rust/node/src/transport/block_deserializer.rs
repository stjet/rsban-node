use std::sync::{Arc, Mutex};

use super::{Socket, SocketExtensions};
use crate::utils::ErrorCode;
use num_traits::FromPrimitive;
use rsnano_core::{
    deserialize_block_enum_with_type, serialized_block_size, utils::StreamAdapter, BlockEnum,
    BlockType,
};

pub struct BlockDeserializer {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl BlockDeserializer {
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(vec![0; 256])),
        }
    }

    pub fn read(
        &self,
        socket: &Arc<Socket>,
        callback: Box<dyn FnOnce(ErrorCode, Option<BlockEnum>) + Send + 'static>,
    ) {
        let buffer_clone = Arc::clone(&self.buffer);
        let socket_clone = Arc::clone(socket);
        socket.async_read(
            Arc::clone(&self.buffer),
            1,
            Box::new(move |ec, _len| {
                if ec.is_err() {
                    callback(ec, None);
                } else {
                    received_type(buffer_clone, &socket_clone, callback);
                }
            }),
        );
    }
}

fn received_type(
    buffer: Arc<Mutex<Vec<u8>>>,
    socket: &Arc<Socket>,
    callback: Box<dyn FnOnce(ErrorCode, Option<BlockEnum>) + Send + 'static>,
) {
    let block_type_byte = {
        let guard = buffer.lock().unwrap();
        guard[0]
    };

    let buffer_clone = Arc::clone(&buffer);

    match BlockType::from_u8(block_type_byte) {
        Some(BlockType::NotABlock) | Some(BlockType::Invalid) => callback(ErrorCode::fault(), None),
        Some(block_type) => {
            let block_size = serialized_block_size(block_type);
            socket.async_read(
                buffer,
                block_size,
                Box::new(move |ec, len| {
                    if ec.is_err() {
                        callback(ErrorCode::fault(), None);
                    } else {
                        let guard = buffer_clone.lock().unwrap();
                        let mut stream = StreamAdapter::new(&guard[..len]);
                        let result = deserialize_block_enum_with_type(block_type, &mut stream);
                        drop(guard);
                        match result {
                            Ok(block) => callback(ErrorCode::new(), Some(block)),
                            Err(_) => callback(ErrorCode::fault(), None),
                        }
                    }
                }),
            );
        }
        None => callback(ErrorCode::fault(), None),
    }
}
