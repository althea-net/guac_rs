use channel_client::types::{NewChannelTx, UpdateTx};
use clarity::abi::{encode_call, Token};
use clarity::Transaction;
use clarity::{Address, BigEndianInt, Signature};
use std::io::Cursor;

use crypto::CryptoService;
use CRYPTO;

pub struct Fullnode {
    pub address: Address,
    pub url: String,
}

fn create_update_tx(update: UpdateTx) -> Transaction {
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

pub fn create_new_channel_tx(
    contract: Address,
    network_id: Option<u64>,
    update: NewChannelTx,
) -> Transaction {
    let challenge: [u8; 32] = update.challenge.into();

    let data = encode_call(
        "openChannel(address,address,uint256,uint256)",
        &[
            // to
            update.to.into(),
            // tokenContract (we use ETH)
            Address::default().into(),
            // tokenAmount
            0u32.into(),
            // SigA
            Token::Bytes(challenge.to_vec().into()),
        ],
    );
    Transaction {
        to: contract,
        // action: Action::Call(Address::default()),
        // TODO: Get nonce from eth full node
        nonce: 42u32.into(),
        // TODO: set this semi automatically
        gas_price: 1_000_000_000u64.into(),
        // TODO: find out how much gas this contract acutally takes
        gas_limit: 21_000u64.into(),
        value: update.deposit.into(),
        data,
        signature: None,
    }.sign(&CRYPTO.secret(), None)
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
    let tx = create_new_channel_tx(
        Address::default(),
        None,
        NewChannelTx {
            to: "0x000000000000000000000000000000000000007b"
                .parse()
                .expect("Unable to parse address"),
            challenge: 23u32.into(),
            deposit: 100u32.into(),
        },
    );
    trace!("tx: {:?}", tx);
}
