use channel_client::types::{NewChannelTx, UpdateTx};
use clarity::Transaction;
use clarity::{Address, BigEndianInt, Signature};
use ethabi::{Contract, Token};
use std::io::Cursor;

use crypto::CryptoService;
use CRYPTO;

pub struct Fullnode {
    pub address: Address,
    pub url: String,
}

lazy_static! {
    static ref ABI: Contract = get_ethcalate_abi();
}

fn get_ethcalate_abi() -> Contract {
    let abi_bytes = include_bytes!("abi/ethcalate-bidirectional-erc20-single-abi.json");
    let c = Cursor::new(abi_bytes.to_vec());

    return Contract::load(c).unwrap();
}

fn create_update_tx(update: UpdateTx) -> Transaction {
    let channel_id: [u8; 32] = update.channel_id.into();
    unimplemented!();

    // let data = ABI
    //     .function("updateState")
    //     .unwrap()
    //     .encode_input(&[
    //         // channelId
    //         Token::FixedBytes(channel_id.to_vec()),
    //         // nonce
    //         Token::Uint(update.nonce),
    //         // balanceA
    //         Token::Uint(update.balance_a),
    //         // balanceB
    //         Token::Uint(update.balance_b),
    //         // SigA
    //         Token::String(update.signature_a.unwrap().to_string()),
    //         // SigB
    //         Token::String(update.signature_b.unwrap().to_string()),
    //     ]).unwrap();

    // Transaction {
    //     action: Action::Call(Address::default()),
    //     // TODO: Get nonce from eth full node
    //     nonce: BigEndianInt::from(42),
    //     // TODO: set this semi automatically
    //     gas_price: BigEndianInt::from(3000),
    //     // TODO: find out how much gas this contract acutally takes
    //     gas: BigEndianInt::from(50_000),
    //     value: BigEndianInt::from(0),
    //     data,
    // }.sign(&CRYPTO.secret(), None)
}

fn create_new_channel_tx(update: NewChannelTx) -> Transaction {
    unimplemented!();
    // let data = ABI
    //     .function("openChannel")
    //     .unwrap()
    //     .encode_input(&[
    //         // to
    //         Token::Address((*update.to).into()),
    //         // tokenContract
    //         Token::Address(Address::default()),
    //         // tokenAmount
    //         Token::Uint(BigEndianInt::from(0)),
    //         // SigA
    //         Token::Uint(update.challenge),
    //     ]).unwrap();

    // Transaction {
    //     to: Address::default(),
    //     // action: Action::Call(Address::default()),
    //     // TODO: Get nonce from eth full node
    //     nonce: BigEndianInt::from(42),
    //     // TODO: set this semi automatically
    //     gas_price: BigEndianInt::from(3000),
    //     // TODO: find out how much gas this contract acutally takes
    //     gas_limit: BigEndianInt::from(50_000),
    //     value: update.deposit.into(),
    //     data,
    // }.sign(&CRYPTO.secret(), None)
    // Transaction::new
}

#[test]
fn test_abi_parse() {
    // just use the lazy static
    &*ABI;
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
    let tx = create_new_channel_tx(NewChannelTx {
        to: "0x0000000000000000000000000000007b".parse().unwrap(),
        challenge: 23u32.into(),
        deposit: 100u32.into(),
    });
    trace!("tx: {:?}", tx);
}
