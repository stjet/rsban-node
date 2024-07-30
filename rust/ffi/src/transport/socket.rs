use crate::{
    utils::{AsyncRuntimeHandle, ThreadPoolHandle},
    ErrorCodeDto, StatHandle,
};
use num::FromPrimitive;
use rsnano_node::{
    stats::SocketStats,
    transport::{alive_sockets, Socket, SocketBuilder},
    utils::ErrorCode,
};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6},
    ops::Deref,
    sync::Arc,
    time::Duration,
};
use tracing::debug;

pub struct SocketHandle(pub Arc<Socket>);

impl SocketHandle {
    pub fn new(socket: Arc<Socket>) -> *mut SocketHandle {
        Box::into_raw(Box::new(SocketHandle(socket)))
    }
}

impl Deref for SocketHandle {
    type Target = Arc<Socket>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_sockets_alive() -> usize {
    alive_sockets()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_create(
    endpoint_type: u8,
    stats_handle: *mut StatHandle,
    thread_pool: &ThreadPoolHandle,
    default_timeout_s: u64,
    silent_connection_tolerance_time_s: u64,
    idle_timeout_s: u64,
    max_write_queue_len: usize,
    async_rt: &AsyncRuntimeHandle,
) -> *mut SocketHandle {
    let endpoint_type = FromPrimitive::from_u8(endpoint_type).unwrap();
    let thread_pool = thread_pool.0.clone();
    let stats = (*stats_handle).deref().clone();

    let socket_stats = Arc::new(SocketStats::new(stats));

    let runtime = Arc::downgrade(&async_rt.0);
    let socket = SocketBuilder::new(endpoint_type, thread_pool, runtime)
        .default_timeout(Duration::from_secs(default_timeout_s))
        .silent_connection_tolerance_time(Duration::from_secs(silent_connection_tolerance_time_s))
        .idle_timeout(Duration::from_secs(idle_timeout_s))
        .observer(socket_stats)
        .max_write_queue_len(max_write_queue_len)
        .finish();
    debug!(socket_id = socket.socket_id, "Socket created from FFI");

    SocketHandle::new(socket)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_destroy(handle: *mut SocketHandle) {
    drop(Box::from_raw(handle))
}

pub struct AsyncWriteCallbackHandle(Option<Box<dyn FnOnce(ErrorCode, usize)>>);

#[no_mangle]
pub unsafe extern "C" fn rsn_async_write_callback_execute(
    callback: *mut AsyncWriteCallbackHandle,
    ec: *const ErrorCodeDto,
    size: usize,
) {
    let error_code = ErrorCode::from(&*ec);
    if let Some(cb) = (*callback).0.take() {
        cb(error_code, size);
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_async_write_callback_destroy(callback: *mut AsyncWriteCallbackHandle) {
    drop(Box::from_raw(callback))
}

#[derive(Clone)]
#[repr(C)]
pub struct EndpointDto {
    pub bytes: [u8; 16],
    pub port: u16,
    pub v6: bool,
}

impl EndpointDto {
    pub fn new() -> EndpointDto {
        EndpointDto {
            bytes: [0; 16],
            port: 0,
            v6: false,
        }
    }
}

impl Default for EndpointDto {
    fn default() -> Self {
        Self::new()
    }
}

impl From<&EndpointDto> for SocketAddrV6 {
    fn from(dto: &EndpointDto) -> Self {
        if dto.v6 {
            SocketAddrV6::new(Ipv6Addr::from(dto.bytes), dto.port, 0, 0)
        } else {
            panic!("not a v6 ip address")
        }
    }
}

impl From<&SocketAddrV6> for EndpointDto {
    fn from(value: &SocketAddrV6) -> Self {
        Self {
            bytes: value.ip().octets(),
            port: value.port(),
            v6: true,
        }
    }
}

impl From<SocketAddrV6> for EndpointDto {
    fn from(value: SocketAddrV6) -> Self {
        Self {
            bytes: value.ip().octets(),
            port: value.port(),
            v6: true,
        }
    }
}

impl From<&EndpointDto> for SocketAddr {
    fn from(dto: &EndpointDto) -> Self {
        let ip = if dto.v6 {
            IpAddr::V6(Ipv6Addr::from(dto.bytes))
        } else {
            let mut bytes = [0; 4];
            bytes.copy_from_slice(&dto.bytes[..4]);
            IpAddr::V4(Ipv4Addr::from(bytes))
        };

        SocketAddr::new(ip, dto.port)
    }
}

impl From<&SocketAddr> for EndpointDto {
    fn from(addr: &SocketAddr) -> Self {
        match addr {
            SocketAddr::V4(a) => {
                let mut dto = EndpointDto {
                    bytes: [0; 16],
                    port: a.port(),
                    v6: false,
                };
                dto.bytes[..4].copy_from_slice(&a.ip().octets());
                dto
            }
            SocketAddr::V6(a) => EndpointDto {
                bytes: a.ip().octets(),
                port: a.port(),
                v6: true,
            },
        }
    }
}

impl From<SocketAddr> for EndpointDto {
    fn from(addr: SocketAddr) -> Self {
        EndpointDto::from(&addr)
    }
}
