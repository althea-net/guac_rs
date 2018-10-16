use channel_client::types::UpdateTx;
use clarity::abi::encode_tokens;
use clarity::abi::{encode_call, Token};
use clarity::Transaction;
use clarity::{Address, BigEndianInt, Signature};
use std::io::Cursor;

use crypto::CryptoService;
use CRYPTO;

/// An alias for a channel ID in a raw bytes form
pub type ChannelId = [u8; 32];

pub struct Fullnode {
    pub address: Address,
    pub url: String,
}

pub fn create_update_tx(update: UpdateTx) -> Transaction {
    let channel_id: [u8; 32] = update.channel_id.into();
    let nonce: [u8; 32] = update.nonce.into();
    let balance_a: [u8; 32] = update.balance_a.into();
    let balance_b: [u8; 32] = update.balance_b.into();
    let signature_a = update.signature_a.unwrap().to_string();
    let signature_b = update.signature_b.unwrap().to_string();
    let data = encode_call(
        "updateState(bytes32,uint256,uint256,uint256,string,string)",
        &[
            // channelId
            Token::Bytes(channel_id.to_vec()),
            // nonce
            Token::Bytes(nonce.to_vec()),
            // balanceA
            Token::Bytes(balance_a.to_vec()),
            // balanceB
            Token::Bytes(balance_b.to_vec()),
            // SigA
            signature_a.as_str().into(),
            // SigB
            signature_b.as_str().into(),
        ],
    );

    Transaction {
        to: Address::default(),
        nonce: BigEndianInt::from(42u32),
        // TODO: set this semi automatically
        gas_price: BigEndianInt::from(3000u32),
        // TODO: find out how much gas this contract acutally takes
        gas_limit: BigEndianInt::from(50_000u32),
        value: BigEndianInt::from(0u32),
        data,
        signature: None,
    }.sign(&CRYPTO.secret(), None)
}

/// Creates a payload for "openChannel" contract call.
///
/// * `to` - Who is expected to be join on the other side of the channel.
/// * `challenge` - A channel challenge which should be unique.
pub fn create_open_channel_payload(to: Address, challenge: BigEndianInt) -> Vec<u8> {
    let challenge: [u8; 32] = challenge.into();

    encode_call(
        "openChannel(address,address,uint256,uint256)",
        &[
            // to
            to.into(),
            // tokenContract (we use ETH)
            Address::default().into(),
            // tokenAmount
            0u32.into(),
            // SigA
            Token::Bytes(challenge.to_vec().into()),
        ],
    )
}

/// Creates a payload for "joinChannel" contract call.
///
/// * `channel_id` - A valid channel ID
pub fn create_join_channel_payload(channel_id: ChannelId) -> Vec<u8> {
    encode_call(
        "joinChannel(bytes32,uint256)",
        &[
            // id
            Token::Bytes(channel_id.to_vec().into()),
            // tokenAmount
            0u32.into(), // should use `msg.value` ^
        ],
    )
}

/// Create a data that should be signed with a private key
/// and the signed data should be passed as a Signature to
/// create_update_channel_payload.
pub fn create_signature_data(
    channel_id: ChannelId,
    nonce: BigEndianInt,
    balance_a: BigEndianInt,
    balance_b: BigEndianInt,
) -> Vec<u8> {
    encode_tokens(&[
        Token::Bytes(channel_id.to_vec()),
        nonce.into(),
        balance_a.into(),
        balance_b.into(),
    ])
}

pub fn create_update_channel_payload(
    channel_id: ChannelId,
    nonce: BigEndianInt,
    balance_a: BigEndianInt,
    balance_b: BigEndianInt,
    sig_a: Signature,
    sig_b: Signature,
) -> Vec<u8> {
    encode_call(
        "updateState(bytes32,uint256,uint256,uint256,string,string)",
        &[
            // channelId
            Token::Bytes(channel_id.to_vec()),
            // nonce
            nonce.into(),
            // balanceA
            balance_a.into(),
            // balanceB
            balance_b.into(),
            // sigA
            sig_a.to_string().as_str().into(),
            // sigB
            sig_b.to_string().as_str().into(),
        ],
    )
}

#[test]
fn test_create_update_tx() {
    let tx = create_update_tx(UpdateTx {
        nonce: 0u32.into(),
        balance_a: 23u32.into(),
        balance_b: 23u32.into(),
        channel_id: 11u32.into(),
        signature_a: Some(Signature::new(1u32.into(), 2u32.into(), 3u32.into())),
        signature_b: Some(Signature::new(4u32.into(), 5u32.into(), 6u32.into())),
    });
    trace!("tx: {:?}", tx);
}

#[test]
fn test_new_channel_tx() {
    let data = create_open_channel_payload(Address::default(), "12345".parse().unwrap());
    trace!("payload: {:?}", data);
}

#[test]
fn test_join_channel_tx() {
    let data = create_join_channel_payload([0u8; 32]);
    assert!(data.len() > 0);
}
