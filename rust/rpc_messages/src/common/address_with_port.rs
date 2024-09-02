use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AddressWithPortArg {
    pub address: String,
    pub port: u16,
}
