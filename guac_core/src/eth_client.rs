use althea_types::EthAddress;
use channel_client::types::{NewChannelTx, UpdateTx};
use ethabi::{Contract, Token};
use ethcore_transaction::{Action, SignedTransaction, Transaction};
use ethereum_types::{Address, U256};
use std::io::Cursor;

use crypto::CryptoService;
use CRYPTO;

pub struct Fullnode {
    pub address: EthAddress,
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

fn create_update_tx(update: UpdateTx) -> SignedTransaction {
    let channel_id: [u8; 32] = update.channel_id.into();

    let data = ABI
        .function("updateState")
        .unwrap()
        .encode_input(&[
            // channelId
            Token::FixedBytes(channel_id.to_vec()),
            // nonce
            Token::Uint(update.nonce),
            // balanceA
            Token::Uint(update.balance_a),
            // balanceB
            Token::Uint(update.balance_b),
            // SigA
            Token::String(update.signature_a.unwrap().to_string()),
            // SigB
            Token::String(update.signature_b.unwrap().to_string()),
        ])
        .unwrap();

    Transaction {
        action: Action::Call(Address::default()),
        // TODO: Get nonce from eth full node
        nonce: U256::from(42),
        // TODO: set this semi automatically
        gas_price: U256::from(3000),
        // TODO: find out how much gas this contract acutally takes
        gas: U256::from(50_000),
        value: U256::from(0),
        data,
    }.sign(&CRYPTO.own_secret(), None)
}

fn create_new_channel_tx(update: NewChannelTx) -> SignedTransaction {
    let data = ABI
        .function("openChannel")
        .unwrap()
        .encode_input(&[
            // to
            Token::Address((*update.to).into()),
            // tokenContract
            Token::Address(Address::default()),
            // tokenAmount
            Token::Uint(U256::from(0)),
            // SigA
            Token::Uint(update.challenge),
        ])
        .unwrap();

    Transaction {
        action: Action::Call(Address::default()),
        // TODO: Get nonce from eth full node
        nonce: U256::from(42),
        // TODO: set this semi automatically
        gas_price: U256::from(3000),
        // TODO: find out how much gas this contract acutally takes
        gas: U256::from(50_000),
        value: update.deposit.into(),
        data,
    }.sign(&CRYPTO.own_secret(), None)
}

#[test]
fn test_abi_parse() {
    // just use the lazy static
    &*ABI;
}

#[test]
fn test_create_update_tx() {
    let tx = create_update_tx(UpdateTx {
        nonce: 0.into(),
        balance_a: 23.into(),
        balance_b: 23.into(),
        channel_id: 11.into(),
        signature_a: Some(1.into()),
        signature_b: Some(2.into()),
    });
    println!("tx: {:?}", tx);
}

#[test]
fn test_new_channel_tx() {
    let tx = create_new_channel_tx(NewChannelTx {
        to: 11.into(),
        challenge: 23.into(),
        deposit: 100.into(),
    });
    println!("tx: {:?}", tx);
}
