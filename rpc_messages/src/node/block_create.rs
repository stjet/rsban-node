use crate::{
    common::{BlockTypeDto, WorkVersionDto},
    RpcCommand, RpcU64,
};
use rsnano_core::{Account, Amount, BlockHash, JsonBlock, Link, RawKey, WalletId, WorkNonce};
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn block_create(block_create_args: BlockCreateArgs) -> Self {
        Self::BlockCreate(block_create_args)
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BlockCreateArgs {
    #[serde(rename = "type")]
    pub block_type: BlockTypeDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<Amount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<RawKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet: Option<WalletId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<Account>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<BlockHash>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<Account>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub representative: Option<Account>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<Link>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<BlockHash>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work: Option<WorkNonce>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<WorkVersionDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<RpcU64>,
}

impl BlockCreateArgs {
    pub fn new(
        block_type: BlockTypeDto,
        balance: Option<Amount>,
        key: Option<RawKey>,
        wallet: Option<WalletId>,
        account: Option<Account>,
        source: Option<BlockHash>,
        destination: Option<Account>,
        representative: Option<Account>,
        link: Option<Link>,
        previous: Option<BlockHash>,
        work: Option<WorkNonce>,
        version: Option<WorkVersionDto>,
        difficulty: Option<u64>,
    ) -> Self {
        Self {
            block_type,
            balance,
            key,
            wallet,
            account,
            source,
            destination,
            representative,
            link,
            previous,
            work,
            version,
            difficulty: difficulty.map(|i| i.into()),
        }
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BlockCreateResponse {
    pub hash: BlockHash,
    pub difficulty: WorkNonce,
    pub block: JsonBlock,
}

impl BlockCreateResponse {
    pub fn new(hash: BlockHash, difficulty: WorkNonce, block: JsonBlock) -> Self {
        Self {
            hash,
            difficulty,
            block,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::{Block, PrivateKey, PublicKey, RawKey};
    use serde_json::json;

    #[test]
    fn serialize_block_create_command() {
        // Create a test StateBlock instance
        let balance = Amount::from(123);
        let account = Account::from(42);
        let representative = PublicKey::from(1);
        let link = Link::from(2);
        let work = 4;
        let previous = BlockHash::from(3);
        let key_pair = PrivateKey::new();
        let raw_key = RawKey::from(key_pair.private_key());

        // Create BlockCreateArgs using the test StateBlock data
        let block_create_args = BlockCreateArgs {
            block_type: BlockTypeDto::State,
            balance: Some(balance),
            key: Some(raw_key),
            wallet: None,
            account: Some(account),
            source: None,
            destination: None,
            representative: Some(representative.as_account()),
            link: Some(link),
            previous: Some(previous),
            work: Some(WorkNonce::from(work)),
            version: Some(WorkVersionDto::Work1),
            difficulty: None,
        };

        // Create the RpcCommand
        let command = RpcCommand::block_create(block_create_args);

        // Serialize the command to JSON
        let serialized = serde_json::to_string_pretty(&command).unwrap();

        // Expected JSON
        let expected_json = json!({
            "action": "block_create",
            "type": "state",
            "balance": balance.to_string_dec(),
            "key": raw_key.encode_hex(),
            "account": account.encode_account(),
            "representative": representative.as_account(),
            "link": link.encode_hex(),
            "previous": previous.encode_hex(),
            "work": format!("{:016X}", work),
            "version": "work1"
        });

        // Deserialize the expected JSON to a Value
        let expected_value: serde_json::Value = expected_json;

        // Deserialize the serialized command to a Value
        let serialized_value: serde_json::Value = serde_json::from_str(&serialized).unwrap();

        // Assert that the serialized command matches the expected JSON
        assert_eq!(serialized_value, expected_value);
    }

    #[test]
    fn deserialize_block_create_command() {
        let json = r#"{
    "action": "block_create",
    "json_block": "true",
    "type": "state",
    "balance": "1000000000000000000000000000000",
    "key": "0000000000000000000000000000000000000000000000000000000000000002",
    "representative": "nano_1hza3f7wiiqa7ig3jczyxj5yo86yegcmqk3criaz838j91sxcckpfhbhhra1",
    "link": "19D3D919475DEED4696B5D13018151D1AF88B2BD3BCFF048B45031C1F36D1858",
    "previous": "F47B23107E5F34B2CE06F562B5C435DF72A533251CB414C51B2B62A8F63A00E4",
    "account": "nano_1hza3f7wiiqa7ig3jczyxj5yo86yegcmqk3criaz838j91sxcckpfhbhhra1",
    "work": "0000000000000123",
    "version": "work1"
}"#;

        let command: RpcCommand = serde_json::from_str(json).unwrap();

        let expected_command = RpcCommand::block_create(BlockCreateArgs {
            block_type: BlockTypeDto::State,
            balance: Some(Amount::nano(1)),
            key: Some(RawKey::decode_hex("2").unwrap()),
            wallet: None,
            account: Some(
                Account::decode_account(
                    "nano_1hza3f7wiiqa7ig3jczyxj5yo86yegcmqk3criaz838j91sxcckpfhbhhra1",
                )
                .unwrap(),
            ),
            source: None,
            destination: None,
            representative: Some(
                Account::decode_account(
                    "nano_1hza3f7wiiqa7ig3jczyxj5yo86yegcmqk3criaz838j91sxcckpfhbhhra1",
                )
                .unwrap(),
            ),
            link: Some(
                Link::decode_hex(
                    "19D3D919475DEED4696B5D13018151D1AF88B2BD3BCFF048B45031C1F36D1858",
                )
                .unwrap(),
            ),
            previous: Some(
                BlockHash::decode_hex(
                    "F47B23107E5F34B2CE06F562B5C435DF72A533251CB414C51B2B62A8F63A00E4",
                )
                .unwrap(),
            ),
            work: Some(WorkNonce::from(0x123)),
            version: Some(WorkVersionDto::Work1),
            difficulty: None,
        });

        assert_eq!(command, expected_command);
    }

    #[test]
    fn serialize_block_create_response() {
        let block = Block::new_test_instance();
        let dto = BlockCreateResponse::new(
            BlockHash::from(123),
            WorkNonce::from(456),
            block.json_representation(),
        );

        let serialized = serde_json::to_string_pretty(&dto).unwrap();
        let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();

        let expected_json = json!({
            "hash": dto.hash,
            "difficulty": dto.difficulty,
            "block": block.json_representation()
        });

        assert_eq!(deserialized, expected_json);
    }

    #[test]
    fn deserialize_block_create_dto() {
        let hash = BlockHash::from(123);
        let block = Block::new_test_instance();

        let json = json!({
            "hash": hash,
            "difficulty": "000000000000000A",
            "block": block.json_representation()
        });

        let json_string = serde_json::to_string(&json).unwrap();
        let dto: BlockCreateResponse = serde_json::from_str(&json_string).unwrap();

        assert_eq!(dto.hash, hash);
        assert_eq!(dto.difficulty, 10.into());
        assert_eq!(dto.block, block.json_representation());
    }
}
