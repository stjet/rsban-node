pub struct TcpStream {
    stream: tokio::net::TcpStream,
}

impl TcpStream {
    pub fn new(stream: tokio::net::TcpStream) -> Self {
        Self { stream }
    }

    pub async fn readable(&self) -> tokio::io::Result<()> {
        self.stream.readable().await
    }

    pub fn try_read(&self, buf: &mut [u8]) -> tokio::io::Result<usize> {
        self.stream.try_read(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::ErrorKind,
        net::{IpAddr, Ipv4Addr, SocketAddr},
    };
    use tokio::{net::TcpListener, spawn};

    #[tokio::test]
    async fn connects_to_real_server() {
        let endpoint = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8088);
        start_test_tcp_server(endpoint).await;

        let tokio_stream = tokio::net::TcpStream::connect("127.0.0.1:50000")
            .await
            .unwrap();

        let stream = TcpStream::new(tokio_stream);
        let mut buf = [0; 3];
        loop {
            stream.readable().await.unwrap();
            match stream.try_read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    break;
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    panic!("unexpected error when reading {:?}", e);
                }
            }
        }
        assert_eq!(buf, [1, 2, 3]);
    }

    async fn start_test_tcp_server(endpoint: SocketAddr) {
        let listener = TcpListener::bind(endpoint).await.unwrap();

        spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            loop {
                socket.writable().await.unwrap();
                match socket.try_write(&[1, 2, 3]) {
                    Ok(_) => {
                        break;
                    }
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                        continue;
                    }
                    Err(e) => {
                        panic!("unexpected error: {:?}", e);
                    }
                }
            }
        });
    }
}
