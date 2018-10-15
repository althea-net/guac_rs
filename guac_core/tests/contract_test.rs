extern crate clarity;
extern crate guac_core;
extern crate web3;
#[macro_use]
extern crate lazy_static;
extern crate rand;
#[macro_use]
extern crate failure;

use clarity::abi::{derive_signature, encode_call, encode_tokens, Token};
use clarity::{Address, BigEndianInt, PrivateKey, Transaction};
use failure::Error;
use guac_core::channel_client::channel_manager::ChannelManager;
use guac_core::crypto::CryptoService;
use guac_core::crypto::CRYPTO;
use guac_core::eth_client::create_open_channel_payload;
use rand::{OsRng, Rng};
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

/// A handle that contains event loop instance and a web3 instance
///
/// EventLoop has to live at least as long as the "Web3" object, or
/// otherwise calls will fail. We achieve this by implementing a Deref
/// trait that would return a borrowed Web3 object.
struct Web3Handle(EventLoopHandle, Web3<Http>);

impl Deref for Web3Handle {
    type Target = Web3<Http>;
    fn deref(&self) -> &Web3<Http> {
        &self.1
    }
}

/// This function verifies if a web3 transport can be safely created.
fn make_web3() -> Option<Web3Handle> {
    let address = env::var("GANACHE_HOST").unwrap_or("http://localhost:8545".to_owned());
    eprintln!("Trying to create a Web3 connection to {:?}", address);
    for counter in 0..30 {
        match web3::transports::Http::new(&address) {
            Ok((evloop, transport)) => {
                // Creating a transport doesn't mean the connection is actually established.
                // Request a list of accounts on the node to verify that connection to the
                // specified network is stable.
                let web3 = Web3::new(transport);
                match web3.eth().accounts().wait() {
                    Ok(accounts) => {
                        println!("Got accounts {:?}", accounts);
                        return Some(Web3Handle(evloop, web3));
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
    static ref CONTRACT_ADDRESS: Address = env::var("CONTRACT_ADDRESS")
        .expect("Unable to obtain contract address. Is $CONTRACT_ADDRESS set properly?")
        .parse()
        .expect("Unable to parse address passed in $CONTRACT_ADDRESS");
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
        .address(vec![CONTRACT_ADDRESS.to_string().parse().unwrap()])
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
    println!("Address {:?}", &*CONTRACT_ADDRESS);
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
    // let alice_cm = ChannelManager::New;
    // let bob_cm = ChannelManager::New;
    let mut cm = ChannelManager::New;

    let action = cm.tick(alice_pk, bob_pk).expect("Doesn't work");
    println!("action {:?}", action);
    println!("cm {:?}", cm);

    let challenge = BigEndianInt::from(42u32);

    // Call openChannel

    let data = create_open_channel_payload(bob.to_public_key().unwrap(), challenge.clone());

    // Get gas price
    let gas_price = WEB3.eth().gas_price().wait().unwrap();
    let gas_price: BigEndianInt = gas_price.to_string().parse().unwrap();
    println!("gas_price {:?}", gas_price);

    let tx = Transaction {
        to: CONTRACT_ADDRESS.clone(),
        // action: Action::Call(Address::default()),
        // TODO: Get nonce from eth full node
        nonce: 0u32.into(),
        // TODO: set this semi automatically
        gas_price: gas_price.clone(),
        // TODO: find out how much gas this contract acutally takes
        gas_limit: 6721975u32.into(), //gas_price.clone() * 2u32.into(),
        value: "1000000000000000000".parse().unwrap(),
        data,
        signature: None,
    }.sign(&CRYPTO.secret(), Some(*NETWORK_ID));

    // Subscribe for ChannelOpen events

    let address_h160: H160 = CONTRACT_ADDRESS.to_string().parse().unwrap();
    let event_future =
        poll_for_event("ChannelOpen(bytes32,address,address,address,uint256,uint256)");
    let call_future = WEB3
        .eth()
        .send_raw_transaction(Bytes::from(tx.to_bytes().unwrap()));

    // Wait for both TX and ChannelOpen event
    let (_tx, log) = call_future.join(event_future).wait().unwrap();

    // Extract ChannelOpen event arguments
    let _token_contract = &log.data.0[0..32];
    let deposit_a: BigEndianInt = log.data.0[32..64].into();
    let challenge: BigEndianInt = log.data.0[64..96].into();
    // let channel_id = log.topics
    let channel_id: [u8; 32] = log.topics[1].into();
    // let channel_id: BigEndianInt = format!("{:?}", log.topics[1]).parse().unwrap();
    assert_eq!(deposit_a, "1000000000000000000".parse().unwrap());
    assert_eq!(challenge, 42u32.into());

    let data = encode_call(
        "joinChannel(bytes32,uint256)",
        &[
            // id
            Token::Bytes(channel_id.to_vec().into()),
            // tokenAmount
            0u32.into(), // should use `msg.value` ^
        ],
    );

    // Switch to bob
    *CRYPTO.secret_mut() = bob.clone();
    assert_eq!(CRYPTO.secret(), bob);

    //
    // Call joinChannel(bytes32 id, uint tokenAmount)
    //
    let tx = Transaction {
        to: CONTRACT_ADDRESS.clone(),
        // action: Action::Call(Address::default()),
        // TODO: Get nonce from eth full node
        nonce: 0u32.into(),
        // TODO: set this semi automatically
        gas_price: gas_price.clone(),
        // TODO: find out how much gas this contract acutally takes
        gas_limit: 6721975u32.into(),
        value: "1000000000000000000".parse().unwrap(),
        data,
        signature: None,
    }.sign(&CRYPTO.secret(), Some(*NETWORK_ID));

    let event_future = poll_for_event("ChannelJoin(bytes32,address,address,uint256,uint256)");

    let call_future = WEB3
        .eth()
        .send_raw_transaction(Bytes::from(tx.to_bytes().unwrap()));

    let (tx, _log) = event_future
        .join(call_future)
        .wait()
        .expect("Unable to wait for call future");
    println!("tx {:?}", tx);
    println!("ChannelJoin {:?}", log);

    // This has to be updated on every state update
    let mut channel_nonce = 0u32;

    //
    // Alice calls updateState
    //
    channel_nonce += 1;
    let data = encode_call(
        "updateState(bytes32,uint256,uint256,uint256,string,string)",
        &[
            // channelId
            Token::Bytes(channel_id.to_vec()),
            // nonce
            channel_nonce.into(),
            // balanceA
            {
                let bal: BigEndianInt = "500000000000000000".parse().unwrap(); // adds 99
                bal.into()
            },
            // balanceB
            {
                let bal: BigEndianInt = "1500000000000000000".parse().unwrap(); // keeps same
                bal.into()
            },
            // sigA
            {
                let payload = encode_tokens(&[
                    Token::Bytes(channel_id.to_vec()),
                    channel_nonce.into(),
                    BigEndianInt::from_str("500000000000000000").unwrap().into(),
                    BigEndianInt::from_str("1500000000000000000")
                        .unwrap()
                        .into(), // keeps same
                ]);
                let sig = alice.sign_msg(&payload);
                sig.to_string().as_str().into()
            },
            // sigB
            {
                let payload = encode_tokens(&[
                    Token::Bytes(channel_id.to_vec()),
                    channel_nonce.into(),
                    BigEndianInt::from_str("500000000000000000").unwrap().into(), // adds 99
                    BigEndianInt::from_str("1500000000000000000")
                        .unwrap()
                        .into(), // keeps same
                ]);
                let sig = alice.sign_msg(&payload);
                sig.to_string().as_str().into()
            },
        ],
    );

    // Switch to alice
    *CRYPTO.secret_mut() = alice.clone();
    assert_eq!(CRYPTO.secret(), alice);

    //
    // Call joinChannel(bytes32 id, uint tokenAmount)
    //
    let tx = Transaction {
        to: CONTRACT_ADDRESS.clone(),
        // action: Action::Call(Address::default()),
        // TODO: Get nonce from eth full node
        nonce: 1u32.into(),
        // TODO: set this semi automatically
        gas_price: gas_price.clone(),
        // TODO: find out how much gas this contract acutally takes
        gas_limit: 6721975u32.into(),
        value: "0".parse().unwrap(),
        data,
        signature: None,
    }.sign(&CRYPTO.secret(), Some(*NETWORK_ID));

    let event_future = poll_for_event("ChannelUpdateState(bytes32,uint256,uint256,uint256)");

    let call_future = WEB3
        .eth()
        .send_raw_transaction(Bytes::from(tx.to_bytes().unwrap()));

    let (tx, log) = call_future
        .join(event_future)
        .wait()
        .expect("Unable to wait for call future");
    println!("tx {:?}", tx);
    println!("ChannelUpdateState {:?}", log);

    //
    // Bob starts a challenge
    //
    let data = encode_call(
        "startChallenge(bytes32)",
        &[
            // channel id
            Token::Bytes(channel_id.to_vec().into()),
        ],
    );

    // Switch to bob
    *CRYPTO.secret_mut() = bob.clone();
    assert_eq!(CRYPTO.secret(), bob);

    //
    // Call startChallenge(bytes32 channelId) public {
    //
    let tx = Transaction {
        to: CONTRACT_ADDRESS.clone(),
        // action: Action::Call(Address::default()),
        // TODO: Get nonce from eth full node
        nonce: 1u32.into(),
        // TODO: set this semi automatically
        gas_price: gas_price.clone(),
        // TODO: find out how much gas this contract acutally takes
        gas_limit: 6721975u32.into(),
        value: "0".parse().unwrap(),
        data,
        signature: None,
    }.sign(&CRYPTO.secret(), Some(*NETWORK_ID));

    let event_future = poll_for_event("ChannelChallenge(bytes32,uint256,address)");

    let call_future = WEB3
        .eth()
        .send_raw_transaction(Bytes::from(tx.to_bytes().unwrap()));

    let (tx, log) = call_future
        .join(event_future)
        .wait()
        .expect("Unable to wait for call future");
    println!("tx {:?}", tx);
    println!("ChannelChallenge {:?}", log);

    //
    // Switch to alice (keep in mind that Bob started the closing challenge)
    //

    let res = WEB3
        .eth()
        .balance(alice.to_public_key().unwrap().as_bytes().into(), None)
        .wait()
        .unwrap();
    println!("Alice {:?}", res);
    let res = WEB3
        .eth()
        .balance(bob.to_public_key().unwrap().as_bytes().into(), None)
        .wait()
        .unwrap();
    println!("Bob {:?}", res);

    *CRYPTO.secret_mut() = alice.clone();
    assert_eq!(CRYPTO.secret(), alice);

    let data = encode_call(
        // function closeChannel(bytes32 channelId) public {
        "closeChannel(bytes32)",
        &[
            // channel id
            Token::Bytes(channel_id.to_vec().into()),
        ],
    );
    let tx = Transaction {
        to: CONTRACT_ADDRESS.clone(),
        // action: Action::Call(Address::default()),
        // TODO: Get nonce from eth full node
        nonce: 2u32.into(),
        // TODO: set this semi automatically
        gas_price: gas_price.clone(),
        // TODO: find out how much gas this contract acutally takes
        gas_limit: 6721975u32.into(),
        value: "0".parse().unwrap(),
        data,
        signature: None,
    }.sign(&CRYPTO.secret(), Some(*NETWORK_ID));

    let event_future = poll_for_event("ChannelClose(bytes32)");

    let call_future = WEB3
        .eth()
        .send_raw_transaction(Bytes::from(tx.to_bytes().unwrap()));

    let (tx, log) = call_future
        .join(event_future)
        .wait()
        .expect("Unable to wait for call future");

    println!("tx {:?}", tx);
    println!("ChannelClose {:?}", log);

    let alice_balance: BigEndianInt = WEB3
        .eth()
        .balance(alice.to_public_key().unwrap().as_bytes().into(), None)
        .wait()
        .unwrap()
        // Convert U256 to BigEndianInt
        .to_string()
        .parse()
        .unwrap();
    println!("Alice {:?}", alice_balance);
    let bob_balance: BigEndianInt = WEB3
        .eth()
        .balance(bob.to_public_key().unwrap().as_bytes().into(), None)
        .wait()
        .unwrap()
        // Convert U256 to BigEndianInt
        .to_string()
        .parse()
        .unwrap();
    println!("Bob {:?}", bob_balance);

    assert!(alice_balance < BigEndianInt::from_str("9500000000000000000").unwrap());
    assert!(bob_balance >= BigEndianInt::from_str("10490000000000000000").unwrap());
}
