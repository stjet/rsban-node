use std::{
    any::Any,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    ops::Deref,
    sync::{Arc, Mutex, Weak},
};

use tokio::net::TcpListener;

use crate::utils::{AsyncRuntime, ErrorCode};

use super::{Socket, TcpStream, TcpStreamFactory};

pub trait TcpSocketFacadeFactory: Send + Sync {
    fn create_tcp_socket(&self) -> Arc<TokioSocketFacade>;
}

pub struct TokioSocketFacade {
    pub runtime: Weak<AsyncRuntime>,
    pub state: Arc<Mutex<TokioSocketState>>,
    // make sure we call the current callback if we close the socket
    pub current_action: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    pub tcp_stream_factory: Arc<TcpStreamFactory>,
}

pub enum TokioSocketState {
    Closed,
    Connecting,
    Client(Arc<TcpStream>),
    Server(Arc<TcpListener>),
}

impl TokioSocketFacade {
    pub fn new(runtime: Arc<AsyncRuntime>, tcp_stream_factory: Arc<TcpStreamFactory>) -> Self {
        Self {
            runtime: Arc::downgrade(&runtime),
            state: Arc::new(Mutex::new(TokioSocketState::Closed)),
            current_action: Mutex::new(None),
            tcp_stream_factory,
        }
    }

    pub fn create(runtime: Arc<AsyncRuntime>) -> Self {
        Self::new(runtime, Arc::new(TcpStreamFactory::new()))
    }

    pub fn create_null() -> Self {
        let runtime = Arc::new(AsyncRuntime::new(
            tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap(),
        ));
        Self::new(runtime, Arc::new(TcpStreamFactory::create_null()))
    }

    pub fn local_endpoint(&self) -> SocketAddr {
        let guard = self.state.lock().unwrap();
        match guard.deref() {
            TokioSocketState::Closed => SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
            TokioSocketState::Client(stream) => stream
                .local_addr()
                .unwrap_or(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0)),
            TokioSocketState::Server(listener) => listener
                .local_addr()
                .unwrap_or(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0)),
            TokioSocketState::Connecting => SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
        }
    }

    pub fn async_accept(
        &self,
        client_socket: &Arc<Socket>,
        callback: Box<dyn FnOnce(SocketAddr, ErrorCode) + Send>,
    ) {
        let callback = Arc::new(Mutex::new(Some(callback)));

        let listener = {
            let guard = self.state.lock().unwrap();
            match guard.deref() {
                TokioSocketState::Server(listener) => Arc::clone(listener),
                _ => {
                    if let Some(cb) = callback.lock().unwrap().take() {
                        cb(
                            SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
                            ErrorCode::fault(),
                        );
                    }
                    return;
                }
            }
        };

        let Some(runtime) = self.runtime.upgrade() else {
            return;
        };
        let runtime_w = Weak::clone(&self.runtime);
        let client_socket = Arc::clone(client_socket);
        runtime.tokio.spawn(async move {
            match listener.accept().await {
                Ok((stream, remote)) => {
                    // wrap the tokio stream in our stream:
                    let stream = TcpStream::new(stream);
                    client_socket.set_stream(stream);
                    let Some(runtime) = runtime_w.upgrade() else {
                        return;
                    };
                    runtime.tokio.spawn_blocking(move || {
                        if let Some(cb) = callback.lock().unwrap().take() {
                            cb(remote, ErrorCode::new());
                        }
                    });
                }
                Err(_) => {
                    let Some(runtime) = runtime_w.upgrade() else {
                        return;
                    };
                    runtime.tokio.spawn_blocking(move || {
                        if let Some(cb) = callback.lock().unwrap().take() {
                            cb(
                                SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
                                ErrorCode::fault(),
                            )
                        }
                    });
                }
            }
        });
    }

    pub fn remote_endpoint(&self) -> Result<SocketAddr, ErrorCode> {
        let guard = self.state.lock().unwrap();
        match guard.deref() {
            TokioSocketState::Client(stream) => stream.peer_addr().map_err(|_| ErrorCode::fault()),
            _ => Err(ErrorCode::fault()),
        }
    }

    pub fn post(&self, f: Box<dyn FnOnce() + Send>) {
        let Some(runtime) = self.runtime.upgrade() else {
            return;
        };
        runtime.tokio.spawn_blocking(move || {
            f();
        });
    }

    pub fn dispatch(&self, f: Box<dyn FnOnce() + Send>) {
        let Some(runtime) = self.runtime.upgrade() else {
            return;
        };
        runtime.tokio.spawn_blocking(move || {
            f();
        });
    }

    pub fn close_acceptor(&self) {
        *self.state.lock().unwrap() = TokioSocketState::Closed;
    }

    pub fn is_acceptor_open(&self) -> bool {
        matches!(
            self.state.lock().unwrap().deref(),
            TokioSocketState::Server(_)
        )
    }

    pub fn as_any(&self) -> &dyn Any {
        self
    }

    pub fn is_open(&self) -> bool {
        let guard = self.state.lock().unwrap();
        match guard.deref() {
            TokioSocketState::Closed => false,
            _ => true,
        }
    }

    pub fn open(&self, endpoint: &SocketAddr) -> ErrorCode {
        {
            let guard = self.state.lock().unwrap();
            debug_assert!(matches!(guard.deref(), TokioSocketState::Closed));
        }
        let Some(runtime) = self.runtime.upgrade() else {
            return ErrorCode::fault();
        };
        match runtime
            .tokio
            .block_on(async move { TcpListener::bind(endpoint).await })
        {
            Ok(listener) => {
                *self.state.lock().unwrap() = TokioSocketState::Server(Arc::new(listener));
                ErrorCode::new()
            }
            Err(_) => ErrorCode::fault(),
        }
    }

    pub fn listening_port(&self) -> u16 {
        let guard = self.state.lock().unwrap();
        match guard.deref() {
            TokioSocketState::Closed => 0,
            TokioSocketState::Client(_) => 0,
            TokioSocketState::Connecting => 0,
            TokioSocketState::Server(listener) => {
                listener.local_addr().map(|a| a.port()).unwrap_or_default()
            }
        }
    }
}

pub struct TokioSocketFacadeFactory {
    runtime: Arc<AsyncRuntime>,
}

impl TokioSocketFacadeFactory {
    pub fn new(runtime: Arc<AsyncRuntime>) -> Self {
        Self { runtime }
    }
}

impl TcpSocketFacadeFactory for TokioSocketFacadeFactory {
    fn create_tcp_socket(&self) -> Arc<TokioSocketFacade> {
        Arc::new(TokioSocketFacade::create(Arc::clone(&self.runtime)))
    }
}
