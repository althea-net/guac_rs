extern crate clarity;
extern crate guac_core;
extern crate web3;
#[macro_use]
extern crate lazy_static;
extern crate rand;

use clarity::abi::{encode_call, Token};
use clarity::{Address, BigEndianInt, PrivateKey, Transaction};
use guac_core::channel_client::channel_manager::ChannelManager;
use guac_core::channel_client::types::NewChannelTx;
use guac_core::crypto::CryptoService;
use guac_core::crypto::CRYPTO;
use guac_core::eth_client::create_new_channel_tx;
use rand::{OsRng, Rng};
use std::env;
use std::ops::Deref;
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

fn make_web3() -> Option<Web3Handle> {
    // TODO: Make it more robust
    let address = env::var("GANACHE_HOST").unwrap_or("http://localhost:8545".to_owned());
    let (evloop, transport) = Http::new(&address).expect("Unable to create HTTP transport");
    Some(Web3Handle(evloop, Web3::new(transport)))
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

#[test]
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

    let challenge: [u8; 32] = BigEndianInt::from(42u32).into();

    let data = encode_call(
        "openChannel(address,address,uint256,uint256)",
        &[
            // to
            bob.to_public_key().unwrap().into(),
            // tokenContract (we use ETH)
            Address::default().into(),
            // tokenAmount
            0u32.into(),
            // SigA
            Token::Bytes(challenge.to_vec().into()),
        ],
    );

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

    // let mut tx = create_new_channel_tx(CONTRACT_ADDRESS.clone(), Some(*NETWORK_ID), NewChannelTx {
    //     to: bob.to_public_key().unwrap(),
    //     challenge: 23u32.into(),
    //     deposit: 100u32.into(),
    // });
    // tx.nonce = 0u32.into();
    // TODO: A proper nonce calculator. With ganache and random keys we don't need to bother.
    // println!("tx: {:?}", tx);

    // Subscribe for ChannelOpen events

    let address_h160: H160 = CONTRACT_ADDRESS.to_string().parse().unwrap();
    // Filter for Hello event in our contract
    let filter = FilterBuilder::default()
        .address(vec![address_h160])
        .topics(
            Some(vec![
                // ChannelOpen(bytes32,address,address,address,uint256,uint256)
                "8cd544d236a3435e48a657e78803d9a0663a99ca62c66c571908f4ab7ecf9586"
                    .parse()
                    .unwrap(),
            ]),
            None,
            None,
            None,
        ).build();

    let event_future = WEB3
        .eth_filter()
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
        }).map_err(|_| ());

    let call_future = WEB3
        .eth()
        .send_raw_transaction(Bytes::from(tx.to_bytes().unwrap()))
        .map_err(|_| ());

    // Wait for both TX and ChannelOpen event
    let (_tx, log) = call_future.join(event_future).wait().unwrap();
    let log = log.unwrap();
    println!("ChannelOpen {:?}", log);

    // Extract ChannelOpen event arguments
    let _token_contract = &log.data.0[0..32];
    let deposit_a: BigEndianInt = log.data.0[32..64].into();
    let challenge: BigEndianInt = log.data.0[64..96].into();
    let channel_id: BigEndianInt = format!("{:?}", log.topics[1]).parse().unwrap();
    assert_eq!(deposit_a, "1000000000000000000".parse().unwrap());
    assert_eq!(challenge, 42u32.into());
}
