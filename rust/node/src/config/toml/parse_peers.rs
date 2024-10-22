use crate::config::Peer;
use std::{net::SocketAddrV6, str::FromStr};

pub(crate) fn parse_peers(input: &[String], default_port: u16) -> Vec<Peer> {
    input
        .iter()
        .filter_map(|s| parse_peer(s, default_port).ok())
        .collect()
}

fn parse_peer(input: &str, default_port: u16) -> anyhow::Result<Peer> {
    if input.contains(']') {
        // IPV6 with port
        let addr = SocketAddrV6::from_str(input)?;
        Ok(Peer::new(addr.ip().to_string(), addr.port()))
    } else if input.contains("::") {
        // IPV6 without port
        Ok(Peer::new(input.to_owned(), default_port))
    } else if input.contains(':') {
        // hostname/ipv4 with port
        let mut values = input.split(':');
        let host = values.next().unwrap().to_owned();
        let port = values
            .next()
            .ok_or_else(|| anyhow!("no port"))?
            .parse::<u16>()?;
        Ok(Peer::new(host, port))
    } else {
        // just hostname/ipv4
        Ok(Peer::new(input.to_owned(), default_port))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn empty() {
        assert!(parse_peers(&[], 42).is_empty());
    }

    #[test]
    fn parse_host_names_only() {
        assert_eq!(
            parse_peers(
                &[
                    "localhost".to_owned(),
                    "127.0.0.1".to_owned(),
                    "my.other.host".to_owned()
                ],
                42
            ),
            [
                Peer::new("localhost", 42),
                Peer::new("127.0.0.1", 42),
                Peer::new("my.other.host", 42)
            ]
        );
    }

    #[test]
    fn parse_ipv6() {
        assert_eq!(parse_peer("::1", 42).unwrap(), Peer::new("::1", 42));
    }

    #[test]
    fn parse_ipv6_with_port() {
        assert_eq!(parse_peer("[::1]:100", 42).unwrap(), Peer::new("::1", 100));
    }

    #[test]
    fn parse_hostname_with_port() {
        assert_eq!(
            parse_peer("my.host.de:100", 42).unwrap(),
            Peer::new("my.host.de", 100)
        );
    }

    #[test]
    fn should_remove_invalid_peers() {
        assert!(parse_peers(&["[asdfasfd]:1".to_owned(), "localhost:".to_owned()], 42).is_empty())
    }
}
