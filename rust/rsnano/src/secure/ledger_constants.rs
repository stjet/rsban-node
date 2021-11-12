use once_cell::sync::Lazy;
use anyhow::Result;

use crate::{blocks::{BlockEnum, deserialize_block_json}, config::{get_env_or_default_string, Networks, WorkThresholds}, numbers::{Account, KeyPair}, utils::SerdePropertyTree};

static DEV_PRIVATE_KEY_DATA: &str =
    "34F0A37AAD20F4A260F0A5B3CB3D7FB50673212263E58A380BC10474BB039CE4";
static DEV_PUBLIC_KEY_DATA: &str =
    "B0311EA55708D6A53C75CDBF88300259C6D018522FE3D4D0A242E431F9E8B6D0"; // xrb_3e3j5tkog48pnny9dmfzj1r16pg8t1e76dz5tmac6iq689wyjfpiij4txtdo
static BETA_PUBLIC_KEY_DATA: &str =
    "259A43ABDB779E97452E188BA3EB951B41C961D3318CA6B925380F4D99F0577A"; // nano_1betagoxpxwykx4kw86dnhosc8t3s7ix8eeentwkcg1hbpez1outjrcyg4n1
static LIVE_PUBLIC_KEY_DATA: &str =
    "E89208DD038FBB269987689621D52292AE9C35941A7484756ECCED92A65093BA"; // xrb_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3
static TEST_PUBLIC_KEY_DATA: Lazy<String> = Lazy::new(|| {
    get_env_or_default_string(
        "NANO_TEST_GENESIS_PUB",
        "45C6FF9D1706D61F0821327752671BDA9F9ED2DA40326B01935AB566FB9E08ED",
    ) // nano_1jg8zygjg3pp5w644emqcbmjqpnzmubfni3kfe1s8pooeuxsw49fdq1mco9j
});

static DEV_GENESIS_DATA: &str = r###"{
	"type": "open",
	"source": "B0311EA55708D6A53C75CDBF88300259C6D018522FE3D4D0A242E431F9E8B6D0",
	"representative": "xrb_3e3j5tkog48pnny9dmfzj1r16pg8t1e76dz5tmac6iq689wyjfpiij4txtdo",
	"account": "xrb_3e3j5tkog48pnny9dmfzj1r16pg8t1e76dz5tmac6iq689wyjfpiij4txtdo",
	"work": "7b42a00ee91d5810",
	"signature": "ECDA914373A2F0CA1296475BAEE40500A7F0A7AD72A5A80C81D7FAB7F6C802B2CC7DB50F5DD0FB25B2EF11761FA7344A158DD5A700B21BD47DE5BD0F63153A02"
    }"###;

static BETA_GENESIS_DATA: &str = r###"{
	"type": "open",
	"source": "259A43ABDB779E97452E188BA3EB951B41C961D3318CA6B925380F4D99F0577A",
	"representative": "nano_1betagoxpxwykx4kw86dnhosc8t3s7ix8eeentwkcg1hbpez1outjrcyg4n1",
	"account": "nano_1betagoxpxwykx4kw86dnhosc8t3s7ix8eeentwkcg1hbpez1outjrcyg4n1",
	"work": "79d4e27dc873c6f2",
	"signature": "4BD7F96F9ED2721BCEE5EAED400EA50AD00524C629AE55E9AFF11220D2C1B00C3D4B3BB770BF67D4F8658023B677F91110193B6C101C2666931F57046A6DB806"
    }"###;

static LIVE_GENESIS_DATA: &str = r###"{
	"type": "open",
	"source": "E89208DD038FBB269987689621D52292AE9C35941A7484756ECCED92A65093BA",
	"representative": "xrb_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3",
	"account": "xrb_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3",
	"work": "62f05417dd3fb691",
	"signature": "9F0C933C8ADE004D808EA1985FA746A7E95BA2A38F867640F53EC8F180BDFE9E2C1268DEAD7C2664F356E37ABA362BC58E46DBA03E523A7B5A19E4B6EB12BB02"
    }"###;

static TEST_GENESIS_DATA: Lazy<String> = Lazy::new(|| {
    get_env_or_default_string(
        "NANO_TEST_GENESIS_BLOCK",
        r###"{
        "type": "open",
        "source": "45C6FF9D1706D61F0821327752671BDA9F9ED2DA40326B01935AB566FB9E08ED",
        "representative": "nano_1jg8zygjg3pp5w644emqcbmjqpnzmubfni3kfe1s8pooeuxsw49fdq1mco9j",
        "account": "nano_1jg8zygjg3pp5w644emqcbmjqpnzmubfni3kfe1s8pooeuxsw49fdq1mco9j",
        "work": "bc1ef279c1a34eb1",
        "signature": "15049467CAEE3EC768639E8E35792399B6078DA763DA4EBA8ECAD33B0EDC4AF2E7403893A5A602EB89B978DABEF1D6606BB00F3C0EE11449232B143B6E07170E"
        }"###,
    )
});

fn parse_block_from_genesis_data (genesis_data: &str) -> Result<BlockEnum>
{
    let ptree = SerdePropertyTree::parse(genesis_data)?;
    deserialize_block_json(&ptree)
}

pub struct LedgerConstants {
    pub work: WorkThresholds,
    pub zero_key: KeyPair,
    pub nano_beta_account: Account,
    pub nano_live_account: Account,
    pub nano_test_account: Account,
}

impl LedgerConstants {
    pub fn new(work: WorkThresholds, _network: Networks) -> Result<Self> {
        Ok(Self {
            work,
            zero_key: KeyPair::zero(),
            nano_beta_account: Account::decode_hex(BETA_PUBLIC_KEY_DATA)?,
            nano_live_account: Account::decode_hex(LIVE_GENESIS_DATA)?,
            nano_test_account: Account::decode_hex(TEST_PUBLIC_KEY_DATA.as_str())?,
        })
    }
}
