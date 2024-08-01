use super::{Socket, SocketExtensions};
use crate::utils::{AsyncRuntime, ErrorCode};
use num_traits::FromPrimitive;
use rsnano_core::{serialized_block_size, utils::BufferReader, BlockEnum, BlockType};
use std::sync::{Arc, Mutex};
use tokio::task::spawn_blocking;

//pub(crate) async fn read_block(socket: &Socket) -> anyhow::Result<BlockEnum> {
//    socket.read_raw(buffer, size)
//}

pub struct BlockDeserializer {
    async_rt: Arc<AsyncRuntime>,
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl BlockDeserializer {
    pub fn new(async_rt: Arc<AsyncRuntime>) -> Self {
        Self {
            async_rt,
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
        self.async_rt.tokio.spawn(async move {
            let result = socket_clone.read_raw(Arc::clone(&buffer_clone), 1).await;

            match result {
                Ok(()) => {
                    received_type(buffer_clone, &socket_clone, callback).await;
                }
                Err(_) => {
                    spawn_blocking(Box::new(move || {
                        callback(ErrorCode::fault(), None);
                    }));
                }
            }
        });
    }
}

async fn received_type(
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
        Some(BlockType::NotABlock) | Some(BlockType::Invalid) => {
            spawn_blocking(Box::new(move || {
                callback(ErrorCode::new(), None);
            }));
        }
        Some(block_type) => {
            let block_size = serialized_block_size(block_type);
            let result = socket.read_raw(buffer, block_size).await;
            match result {
                Ok(()) => {
                    let result = {
                        let guard = buffer_clone.lock().unwrap();
                        let mut stream = BufferReader::new(&guard[..block_size]);
                        BlockEnum::deserialize_block_type(block_type, &mut stream)
                    };

                    spawn_blocking(Box::new(move || match result {
                        Ok(block) => callback(ErrorCode::new(), Some(block)),
                        Err(_) => callback(ErrorCode::fault(), None),
                    }));
                }
                Err(_) => {
                    spawn_blocking(Box::new(move || {
                        callback(ErrorCode::fault(), None);
                    }));
                }
            }
        }
        None => callback(ErrorCode::fault(), None),
    }
}
