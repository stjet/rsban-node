use super::LmdbDatabase;
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct ConfiguredDatabase {
    pub dbi: LmdbDatabase,
    pub db_name: String,
    pub entries: BTreeMap<Vec<u8>, Vec<u8>>,
}

pub static EMPTY_DATABASE: ConfiguredDatabase = ConfiguredDatabase::new_empty();

impl ConfiguredDatabase {
    pub fn new(dbi: LmdbDatabase, name: impl Into<String>) -> Self {
        Self {
            dbi,
            db_name: name.into(),
            entries: BTreeMap::new(),
        }
    }

    const fn new_empty() -> Self {
        Self {
            dbi: LmdbDatabase::new_null(42),
            db_name: String::new(),
            entries: BTreeMap::new(),
        }
    }
}

impl Default for ConfiguredDatabase {
    fn default() -> Self {
        Self {
            dbi: LmdbDatabase::new_null(42),
            db_name: "nulled_database".to_string(),
            entries: Default::default(),
        }
    }
}
