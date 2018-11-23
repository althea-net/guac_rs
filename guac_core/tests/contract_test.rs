extern crate clarity;
extern crate guac_core;
extern crate web3;
#[macro_use]
extern crate lazy_static;
extern crate rand;
#[macro_use]
extern crate failure;
extern crate num256;

use clarity::abi::{derive_signature, encode_call, encode_tokens, Token};
use clarity::{Address, PrivateKey, Transaction};
use failure::Error;
use guac_core::crypto::Config;
use guac_core::crypto::CryptoService;
use guac_core::crypto::CRYPTO;
use guac_core::eth_client::EthClient;
use guac_core::eth_client::{create_signature_data, create_update_channel_payload};
use guac_core::network::Web3Handle;
use guac_core::payment_contract::PaymentContract;
use num256::Uint256;
use rand::{rngs::OsRng, Rng, RngCore};

use std::env;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use std::time;
use web3::futures::future::ok;
use web3::futures::Async;
use web3::futures::{Future, IntoFuture, Stream};
use web3::transports::{EventLoopHandle, Http};
use web3::types::{Bytes, FilterBuilder, Log, TransactionRequest, H160, U256};
use web3::Web3;

fn make_web3() -> Option<Web3Handle> {
    let address = env::var("GANACHE_HOST").unwrap_or("http://localhost:8545".to_owned());
    eprintln!("Trying to create a Web3 connection to {:?}", address);
    for counter in 0..30 {
        match Web3Handle::new(&address) {
            Ok(web3) => {
                // Request a list of accounts on the node to verify that connection to the
                // specified network is stable.
                match web3.eth().accounts().wait() {
                    Ok(accounts) => {
                        println!("Got accounts {:?}", accounts);
                        return Some(web3);
                    }
                    Err(e) => {
                        eprintln!("Unable to retrieve accounts ({}): {}", counter, e);
                        thread::sleep(time::Duration::from_secs(1));
                        continue;
                    }
                }
            }
            Err(e) => {
                eprintln!("Unable to create transport ({}): {}", counter, e);
                thread::sleep(time::Duration::from_secs(1));
                continue;
            }
        }
    }
    None
}

lazy_static! {
    static ref CHANNEL_ADDRESS: Address = env::var("CHANNEL_ADDRESS")
        .expect("Unable to obtain channel manager contract address. Is $CHANNEL_ADDRESS set properly?")
        .parse()
        .expect("Unable to parse address passed in $CHANNEL_ADDRESS");
    static ref WEB3: Web3Handle =
        make_web3().expect("Unable to create a valid transport for Web3 protocol");

    // WEB3.
    static ref NETWORK_ID : u64 = WEB3.net()
            .version()
            .wait()
            .expect("Unable to obtain network ID")
            .parse()
            .expect("Unable to parse network ID");

    static ref ONE_ETH: U256 = "de0b6b3a7640000".parse().unwrap();

    // Choose a seed key which is the first key returned by the network
    static ref SEED_ADDRESS : H160 = WEB3
         .eth()
         .accounts()
         .wait()
         .expect("Unable to retrieve accounts")
         .into_iter()
         .nth(0)
         .expect("Unable to obtain first address from the test network");
}

/// Creates a random private key
fn make_random_key() -> PrivateKey {
    let mut rng = OsRng::new().unwrap();
    let mut data = [0u8; 32];
    rng.fill_bytes(&mut data);

    let res = PrivateKey::from(data);
    debug_assert_ne!(res, PrivateKey::new());
    res
}

/// Crates a private key with a balance of one ETH.
///
/// This is accomplished by creating a random private key, and transferring
/// a 10ETH from a seed address.
fn make_seeded_key() -> PrivateKey {
    let key = make_random_key();
    let tx_req = TransactionRequest {
        from: *SEED_ADDRESS,
        to: Some(key.to_public_key().unwrap().as_bytes().into()),
        gas: None,
        gas_price: Some(0x1.into()),
        value: Some(&*ONE_ETH * 10u32),
        data: None,
        nonce: None,
        condition: None,
    };
    let _res = WEB3.eth().send_transaction(tx_req).wait().unwrap();
    let res = WEB3
        .eth()
        .balance(key.to_public_key().unwrap().as_bytes().into(), None)
        .wait();
    println!("Balance {:?}", res);
    key
}

/// Waits for a single occurence of an event call and returns the log data
fn poll_for_event(event: &str) -> web3::Result<Log> {
    let filter = FilterBuilder::default()
        .address(vec![CHANNEL_ADDRESS.to_string().parse().unwrap()])
        .topics(Some(vec![derive_signature(event).into()]), None, None, None)
        .build();

    Box::new(
        WEB3.eth_filter()
            .create_logs_filter(filter)
            .then(|filter| {
                filter
                    .unwrap()
                    .stream(time::Duration::from_secs(0))
                    .into_future()
                    .map(|(head, _tail)| {
                        // Throw away rest of the stream
                        head
                    })
            }).map_err(|(e, _)| e)
            .map(|maybe_log| maybe_log.expect("Expected log data but None found"))
            .into_future(),
    )
}

#[test]
#[ignore]
fn contract() {
    let cfg = Config {
        address: env::var("GANACHE_HOST").unwrap_or("http://localhost:8545".to_owned()),
        contract: CHANNEL_ADDRESS.clone(),
        secret: "fafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafa"
            .parse()
            .unwrap(),
    };
    CRYPTO.init(&cfg).unwrap();

    let contract: Box<PaymentContract> = Box::new(EthClient::new());

    println!("Address {:?}", &*CHANNEL_ADDRESS);
    println!("Network ID {:?}", &*NETWORK_ID);
    // Set up both parties (alice and bob)
    // they will be used to exchange ETH through channels contract.
    let alice = make_seeded_key();
    // Initialize CRYPTO context by cloning alice's key
    *CRYPTO.secret_mut() = alice.clone();
    assert_eq!(CRYPTO.secret(), alice);

    let bob = make_seeded_key();
    let alice_pk = alice
        .to_public_key()
        .expect("Unable to get Alice's public key");

    println!("Alice PK {:?}", alice_pk.to_string());
    let bob_pk = bob.to_public_key().expect("Unable to get Bob's public key");
    println!("Bob PK {:?}", bob_pk.to_string());
}

#[test]
#[ignore]
fn init_and_query() {
    let cfg = Config {
        address: "http://127.0.0.1:8545".to_string(),
        contract: CHANNEL_ADDRESS.clone(),
        secret: "fafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafa"
            .parse()
            .unwrap(),
    };
    CRYPTO.init(&cfg).unwrap();
    assert_ne!(CRYPTO.web3().eth().accounts().wait().unwrap().len(), 0);

    assert_eq!(CRYPTO.get_network_id().wait().unwrap(), *NETWORK_ID);
    assert_eq!(CRYPTO.get_nonce().wait().unwrap(), Uint256::from(0u64));
    assert_ne!(CRYPTO.get_gas_price().wait().unwrap(), Uint256::from(0u64));
    assert_eq!(
        CRYPTO.get_network_balance().wait().unwrap(),
        Uint256::from(0u64)
    );
}
