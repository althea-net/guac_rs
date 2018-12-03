use clarity::utils::{bytes_to_hex_str, hex_str_to_bytes};
use clarity::Address;
use num256::Uint256;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use std::ops::Deref;

/// Serializes slice of data as "UNFORMATTED DATA" format required
/// by Ethereum JSONRPC API.
///
/// See more https://github.com/ethereum/wiki/wiki/JSON-RPC#hex-value-encoding
pub fn data_serialize<S>(x: &[u8], s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&format!("0x{}", bytes_to_hex_str(x)))
}

/// Deserializes slice of data as "UNFORMATTED DATA" format required
/// by Ethereum JSONRPC API.
///
/// See more https://github.com/ethereum/wiki/wiki/JSON-RPC#hex-value-encoding
pub fn data_deserialize<'de, D>(d: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(d)?;
    hex_str_to_bytes(&s).map_err(serde::de::Error::custom)
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Log {
    /// true when the log was removed, due to a chain reorganization. false if its a valid log.
    pub removed: Option<bool>,
    /// integer of the log index position in the block. null when its pending log.
    #[serde(rename = "logIndex")]
    pub log_index: Option<Uint256>,
    /// integer of the transactions index position log was created from. null when its pending log.
    #[serde(rename = "transactionIndex")]
    pub transaction_index: Option<Uint256>,
    /// hash of the transactions this log was created from. null when its pending log.
    #[serde(rename = "transactionHash")]
    pub transaction_hash: Option<String>,
    /// hash of the block where this log was in. null when its pending. null when its pending log.
    #[serde(rename = "blockHash")]
    pub block_hash: Option<String>,
    /// the block number where this log was in. null when its pending. null when its pending log.
    #[serde(rename = "blockNumber")]
    pub block_number: Option<Uint256>,
    /// 20 Bytes - address from which this log originated.
    pub address: Address,
    /// contains the non-indexed arguments of the log.
    #[serde(
        serialize_with = "data_serialize",
        deserialize_with = "data_deserialize"
    )]
    pub data: Vec<u8>, //
    /// Array of 0 to 4 32 Bytes DATA of indexed log arguments. (In solidity:
    /// The first topic is the hash of the signature of the
    /// event (e.g. Deposit(address,bytes32,uint256)), except you declared
    /// the event with the anonymous specifier.)
    pub topics: Vec<String>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
}

#[derive(Serialize, Default, Debug)]
pub struct NewFilter {
    #[serde(
        rename = "fromBlock",
        skip_serializing_if = "Option::is_none"
    )]
    pub from_block: Option<String>,
    #[serde(rename = "toBlock", skip_serializing_if = "Option::is_none")]
    pub to_block: Option<String>,
    pub address: Vec<Address>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topics: Option<Vec<Option<Vec<Option<String>>>>>,
}

#[derive(Serialize)]
pub struct TransactionRequest {
    //The address the transaction is send from.
    pub from: Address,
    // The address the transaction is directed to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<Address>,
    // Integer of the gas provided for the transaction execution. It will return unused gas.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<Uint256>,
    // Integer of the gasPrice used for each paid gas
    #[serde(rename = "gasPrice")]
    pub gas_price: Option<Uint256>,
    // Integer of the value sent with this transaction
    pub value: Option<Uint256>,
    // The compiled code of a contract OR the hash of the invoked method signature and encoded parameters. For details see Ethereum Contract ABI
    pub data: Option<String>,
    //  This allows to overwrite your own pending transactions that use the same nonce.
    pub nonce: Option<Uint256>,
}

#[test]
fn decode_log() {
    let _res: Vec<Log> = serde_json::from_str(r#"[
        {"logIndex":"0x0",
        "transactionIndex":"0x0",
        "transactionHash":"0xd6785de92c3d55e22a50ef6a37553b1abd4fc710d3662e38369656d4e747662b",
        "blockHash":"0x5d1c0bf2d5d32754f3f9501c9d299beb12447ea2a024e0cb67628979eb6dbf36",
        "blockNumber":"0x53","address":"0xc153bde3ab8a9721b6252dcd1ffa2cb0aa165c1a",
        "data":"0xfd13bb0c43a8e298ee038c1c64d7a93e9653dcab2ff741005d6613ba28f31bd4",
        "topics":["0xa79f57c989b24a51391abba00096b6d17aac193697cbc283ee2ec6570abd3111","0x000000000000000000000000b3b2b9fbf1e8cc9713dbde822eba95fbc4a9f698","0x000000000000000000000000e817f611a758ca765b09b60e2dbcceedaaa5e90c"],
        "type":"mined"}]"#).unwrap();
}
