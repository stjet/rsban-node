use crate::{LmdbDatabase, LmdbEnv, LmdbWriteTransaction, Transaction};
use lmdb::{DatabaseFlags, WriteFlags};
use std::{
    array::TryFromSliceError,
    net::SocketAddrV6,
    ops::Deref,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub struct LmdbPeerStore {
    database: LmdbDatabase,
}

impl LmdbPeerStore {
    pub fn new(env: Arc<LmdbEnv>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("peers"), DatabaseFlags::empty())?;

        Ok(Self { database })
    }

    pub fn database(&self) -> LmdbDatabase {
        self.database
    }

    pub fn put(&self, txn: &mut LmdbWriteTransaction, endpoint: &SocketAddrV6, time: SystemTime) {
        txn.put(
            self.database,
            &EndpointBytes::from(endpoint),
            &TimeBytes::from(time),
            WriteFlags::empty(),
        )
        .unwrap();
    }

    pub fn del(&self, txn: &mut LmdbWriteTransaction, endpoint: &SocketAddrV6) {
        txn.delete(self.database, &EndpointBytes::from(endpoint), None)
            .unwrap();
    }

    pub fn exists(&self, txn: &dyn Transaction, endpoint: &SocketAddrV6) -> bool {
        txn.exists(self.database, &EndpointBytes::from(endpoint))
    }

    pub fn count(&self, txn: &dyn Transaction) -> u64 {
        txn.count(self.database)
    }

    pub fn clear(&self, txn: &mut LmdbWriteTransaction) {
        txn.clear_db(self.database).unwrap();
    }

    pub fn iter<'txn>(
        &self,
        txn: &'txn dyn Transaction,
    ) -> impl Iterator<Item = (SocketAddrV6, SystemTime)> + 'txn {
        txn.open_ro_cursor(self.database)
            .expect("Could not read peer store database")
            .iter_start()
            .map(|i| i.unwrap())
            .map(|(k, v)| {
                let peer: SocketAddrV6 = EndpointBytes::try_from(k).unwrap().into();
                let time: SystemTime = TimeBytes::try_from(v).unwrap().into();
                (peer, time)
            })
    }
}

pub struct EndpointBytes([u8; 18]);

impl Deref for EndpointBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<&[u8]> for EndpointBytes {
    type Error = TryFromSliceError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let buffer: [u8; 18] = value.try_into()?;
        Ok(Self(buffer))
    }
}

impl From<&SocketAddrV6> for EndpointBytes {
    fn from(value: &SocketAddrV6) -> Self {
        let mut bytes = [0; 18];
        let (ip, port) = bytes.split_at_mut(16);
        ip.copy_from_slice(&value.ip().octets());
        port.copy_from_slice(&value.port().to_be_bytes());
        Self(bytes)
    }
}

impl From<EndpointBytes> for SocketAddrV6 {
    fn from(value: EndpointBytes) -> Self {
        let (ip, port) = value.0.split_at(16);
        let ip: [u8; 16] = ip.try_into().unwrap();
        let port: [u8; 2] = port.try_into().unwrap();
        SocketAddrV6::new(ip.into(), u16::from_be_bytes(port), 0, 0)
    }
}

pub struct TimeBytes([u8; 8]);

impl Deref for TimeBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<&[u8]> for TimeBytes {
    type Error = TryFromSliceError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let buffer: [u8; 8] = value.try_into()?;
        Ok(Self(buffer))
    }
}

impl From<SystemTime> for TimeBytes {
    fn from(value: SystemTime) -> Self {
        Self(
            (value
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64)
                .to_be_bytes(),
        )
    }
}

impl From<TimeBytes> for SystemTime {
    fn from(value: TimeBytes) -> Self {
        UNIX_EPOCH + Duration::from_millis(u64::from_be_bytes(value.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeleteEvent, PutEvent};
    use std::{
        net::Ipv6Addr,
        time::{Duration, UNIX_EPOCH},
    };

    #[test]
    fn empty_store() {
        let fixture = Fixture::new();
        let txn = fixture.env.tx_begin_read();
        let store = &fixture.store;
        assert_eq!(store.count(&txn), 0);
        assert_eq!(store.exists(&txn, &TEST_PEER_A), false);
        assert_eq!(store.iter(&txn).next(), None);
    }

    #[test]
    fn add_one_endpoint() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let put_tracker = txn.track_puts();

        let key = TEST_PEER_A;
        let time = UNIX_EPOCH + Duration::from_secs(1261440000);
        fixture.store.put(&mut txn, &key, time);

        assert_eq!(
            put_tracker.output(),
            vec![PutEvent {
                database: LmdbDatabase::new_null(42),
                key: vec![0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 8, 0x3, 0xE8],
                value: 1261440000000u64.to_be_bytes().to_vec(),
                flags: WriteFlags::empty()
            }]
        )
    }

    #[test]
    fn exists() {
        let fixture = Fixture::with_stored_data(vec![TEST_PEER_A.clone(), TEST_PEER_B.clone()]);

        let txn = fixture.env.tx_begin_read();

        assert_eq!(fixture.store.exists(&txn, &TEST_PEER_A), true);
        assert_eq!(fixture.store.exists(&txn, &TEST_PEER_B), true);
        assert_eq!(fixture.store.exists(&txn, &UNKNOWN_PEER), false);
    }

    #[test]
    fn count() {
        let fixture = Fixture::with_stored_data(vec![TEST_PEER_A, TEST_PEER_B]);
        let txn = fixture.env.tx_begin_read();
        assert_eq!(fixture.store.count(&txn), 2);
    }

    #[test]
    fn delete() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let delete_tracker = txn.track_deletions();

        fixture.store.del(&mut txn, &TEST_PEER_A);

        assert_eq!(
            delete_tracker.output(),
            vec![DeleteEvent {
                database: LmdbDatabase::new_null(42),
                key: EndpointBytes::from(&TEST_PEER_A).to_vec()
            }]
        )
    }

    const TEST_PEER_A: SocketAddrV6 =
        SocketAddrV6::new(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8), 1000, 0, 0);

    const TEST_PEER_B: SocketAddrV6 =
        SocketAddrV6::new(Ipv6Addr::new(3, 3, 3, 3, 3, 3, 3, 3), 2000, 0, 0);

    const UNKNOWN_PEER: SocketAddrV6 =
        SocketAddrV6::new(Ipv6Addr::new(4, 4, 4, 4, 4, 4, 4, 4), 4000, 0, 0);

    struct Fixture {
        env: Arc<LmdbEnv>,
        store: LmdbPeerStore,
    }

    impl Fixture {
        fn new() -> Self {
            Self::with_env(LmdbEnv::new_null())
        }

        fn with_stored_data(entries: Vec<SocketAddrV6>) -> Self {
            let mut env = LmdbEnv::new_null_with().database("peers", LmdbDatabase::new_null(42));

            for entry in entries {
                env = env.entry(&EndpointBytes::from(&entry), &[]);
            }

            Self::with_env(env.build().build())
        }

        fn with_env(env: LmdbEnv) -> Self {
            let env = Arc::new(env);
            Self {
                env: env.clone(),
                store: LmdbPeerStore::new(env).unwrap(),
            }
        }
    }
}
