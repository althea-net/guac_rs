extern crate clarity;
extern crate guac_core;
extern crate web3;
#[macro_use]
extern crate lazy_static;
extern crate rand;
#[macro_use]
extern crate failure;
extern crate num256;
extern crate sha3;
use clarity::abi::{derive_signature, encode_call, encode_tokens, Token};
use clarity::{Address, PrivateKey, Signature, Transaction};
use failure::Error;
use guac_core::crypto::Config;
use guac_core::crypto::CryptoService;
use guac_core::crypto::CRYPTO;
use guac_core::eth_client::create_signature_data;
use guac_core::eth_client::EthClient;
use guac_core::network::Web3Handle;
use guac_core::payment_contract::{ChannelId, PaymentContract};
use num256::Uint256;
use rand::{OsRng, Rng};
use sha3::Digest;
use sha3::{Keccak256, Sha3_256};
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

    static ref BLOCK_NUMBER : Uint256 = WEB3
        .eth()
        .block_number()
        .wait()
        .expect("Unable to retrieve block number")
        .to_string()
        .parse()
        .expect("Unable to convert block number");
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

fn create_newchannel_fingerprint(
    secret0: &PrivateKey,
    secret1: &PrivateKey,
    address0: Address,
    address1: Address,
    balance0: Uint256,
    balance1: Uint256,
    expiration: Uint256,
    settling: Uint256,
) -> (Signature, Signature) {
    let (address0, address1, balance0, balance1) = if address0 > address1 {
        (address1, address0, balance1, balance0)
    } else {
        (address0, address1, balance0, balance1)
    };

    assert!(address0 < address1);

    let mut msg = "newChannel".as_bytes().to_vec();
    msg.extend(CHANNEL_ADDRESS.clone().as_bytes());
    msg.extend(address0.as_bytes());
    msg.extend(address1.as_bytes());
    msg.extend(&{
        let data: [u8; 32] = balance0.into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = balance1.into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = expiration.into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = settling.into();
        data
    });

    (secret0.sign_msg(&msg), secret1.sign_msg(&msg))
}

fn create_update_fingerprint(
    secret0: &PrivateKey,
    secret1: &PrivateKey,
    channel_id: ChannelId,
    sequence_number: Uint256,
    balance0: Uint256,
    balance1: Uint256,
) -> (Signature, Signature) {
    // Reorder secret keys as it matters who signs the fingerprint
    let (secret0, secret1) = if secret0.to_public_key().unwrap() > secret1.to_public_key().unwrap()
    {
        (secret1, secret0)
    } else {
        (secret0, secret1)
    };

    let mut msg = "updateState".as_bytes().to_vec();
    msg.extend(CHANNEL_ADDRESS.clone().as_bytes());
    msg.extend(channel_id.to_vec());
    msg.extend(&{
        let data: [u8; 32] = sequence_number.into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = balance0.into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = balance1.into();
        data
    });

    (secret0.sign_msg(&msg), secret1.sign_msg(&msg))
}

fn create_close_fingerprint(
    secret0: &PrivateKey,
    secret1: &PrivateKey,
    channel_id: ChannelId,
    sequence_number: Uint256,
    balance0: Uint256,
    balance1: Uint256,
) -> (Signature, Signature) {
    // Reorder secret keys as it matters who signs the fingerprint
    let (secret0, secret1) = if secret0.to_public_key().unwrap() > secret1.to_public_key().unwrap()
    {
        (secret1, secret0)
    } else {
        (secret0, secret1)
    };

    let mut msg = "closeChannelFast".as_bytes().to_vec();
    msg.extend(CHANNEL_ADDRESS.clone().as_bytes());
    msg.extend(channel_id.to_vec());
    msg.extend(&{
        let data: [u8; 32] = sequence_number.into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = balance0.into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = balance1.into();
        data
    });

    (secret0.sign_msg(&msg), secret1.sign_msg(&msg))
}

fn create_settling_fingerprint(key: &PrivateKey, channel_id: ChannelId) -> Signature {
    let mut msg = "startSettlingPeriod".as_bytes().to_vec();
    msg.extend(CHANNEL_ADDRESS.clone().as_bytes());
    msg.extend(channel_id.to_vec());
    key.sign_msg(&msg)
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

    if alice_pk > bob_pk {
        println!("ALICE_PK > BOB_PK (CASE 1 UNORDERED)");
    } else {
        println!("ALICE_PK < BOB_PK (CASE 1 ORDERED)");
    }

    // Call openChannel

    // Get gas price
    let gas_price = WEB3.eth().gas_price().wait().unwrap();
    let gas_price: Uint256 = gas_price.to_string().parse().unwrap();

    // This has to be updated on every state update
    let mut channel_nonce = 0u32;

    let mut alice_balance: Uint256 = "1000000000000000000".parse().unwrap();
    let mut bob_balance: Uint256 = "0".parse().unwrap();
    let total_balance = alice_balance.clone() + bob_balance.clone();
    println!("total balance {}", total_balance);

    let expiration: Uint256 = (BLOCK_NUMBER.clone() + 100u64).into();
    let settling: Uint256 = 200u64.into();

    contract
        .quick_deposit(alice_balance.clone())
        .wait()
        .unwrap();

    let (sig0, sig1) = create_newchannel_fingerprint(
        &alice,
        &bob,
        alice.to_public_key().unwrap(),
        bob.to_public_key().unwrap(),
        alice_balance.clone(),
        bob_balance.clone(),
        expiration.clone(),
        settling.clone(),
    );

    let fut = contract.new_channel(
        alice.to_public_key().unwrap(),
        bob.to_public_key().unwrap(),
        alice_balance.clone().into(),
        bob_balance.clone().into(),
        sig0,
        sig1,
        expiration.clone().into(),
        settling.clone().into(),
    );

    let channel_id = fut.wait().unwrap();
    assert!(channel_id != [0u8; 32]);
    println!("channel id {:x?}", channel_id);

    // Progress nonce
    channel_nonce += 1;

    let op: Uint256 = "100000000000000000".parse().unwrap();
    // let op : Uint256 = "0".parse().unwrap();
    assert!(op <= alice_balance);
    assert!(op > bob_balance);

    alice_balance -= op.clone();
    bob_balance += op.clone();
    assert_eq!(alice_balance.clone() + bob_balance.clone(), total_balance);

    // Reorder balances based on private key (can't do it inside the method)
    let (balance0, balance1) = if alice.to_public_key().unwrap() > bob.to_public_key().unwrap() {
        (bob_balance.clone(), alice_balance.clone())
    } else {
        (alice_balance.clone(), bob_balance.clone())
    };

    // let alice_balance : Uint256 = "900000000000000000".parse().unwrap();
    // let bob_balance : Uint256 = "100000000000000000".parse().unwrap();
    let (sig_a, sig_b) = create_update_fingerprint(
        &alice,
        &bob,
        channel_id.clone(),
        channel_nonce.clone().into(),
        balance0.clone(),
        balance1.clone(),
    );

    let fut = contract.update_state(
        channel_id,
        channel_nonce.clone().into(),
        balance0.clone(),
        balance1.clone(),
        sig_a,
        sig_b,
    );

    fut.wait().unwrap();

    channel_nonce += 1;

    let sig = create_settling_fingerprint(&alice, channel_id);
    contract
        .start_settling_period(channel_id, sig)
        .wait()
        .unwrap();

    let (sig_a, sig_b) = create_close_fingerprint(
        &alice,
        &bob,
        channel_id.clone(),
        channel_nonce.clone().into(),
        balance0.clone(),
        balance1.clone(),
    );

    let fut = contract.close_channel_fast(
        channel_id,
        channel_nonce.clone().into(),
        balance0.clone(),
        balance1.clone(),
        sig_a,
        sig_b,
    );
    fut.wait().unwrap();

    // contract.close_channel(channel_id).wait().unwrap();
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
