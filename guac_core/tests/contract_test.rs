extern crate clarity;
extern crate guac_core;
#[macro_use]
extern crate lazy_static;
extern crate rand;
#[macro_use]
extern crate failure;
extern crate actix;
extern crate futures;
extern crate futures_executor;
extern crate num256;
extern crate sha3;

use actix::prelude::*;
use actix::SystemRunner;
use clarity::abi::{derive_signature, encode_call, encode_tokens, Token};
use clarity::{Address, PrivateKey, Signature, Transaction};
// use failure::Error;
use futures::future::ok;
use futures::Async;
use futures::{Future, IntoFuture, Stream};
use guac_core::contracts::guac_contract::GuacContract;
use guac_core::contracts::guac_contract::{
    create_close_channel_fast_fingerprint_data, create_new_channel_fingerprint_data,
    create_redraw_fingerprint_data, create_update_fingerprint_data,
    create_update_with_bounty_fingerprint_data,
};
use guac_core::crypto::Config;
use guac_core::crypto::CryptoService;
use guac_core::crypto::CRYPTO;
use guac_core::payment_contract::{ChannelId, PaymentContract};
use guac_core::web3::client::{Web3, Web3Client};
use guac_core::web3::types::TransactionRequest;
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

fn make_web3() -> Option<Web3Client> {
    let address = env::var("GANACHE_HOST").unwrap_or("http://localhost:8545".to_owned());
    eprintln!("Trying to create a Web3 connection to {:?}", address);
    for counter in 0..30 {
        let web3 = Web3Client::new(&address);
        match block_on(web3.eth_accounts()) {
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
    None
}
use futures::{sync::mpsc, Sink};
use std::cell::RefCell;

/// Executes a future on a temporary System and converts
/// a Future into a Result.
fn block_on<R: 'static, E: 'static, F: 'static + Future<Item = R, Error = E>>(
    f: F,
) -> Result<R, E> {
    let sys = System::new("test");
    let (tx, rx) = mpsc::unbounded();
    Arbiter::spawn(
        f.then(move |result| {
            tx
                .send(result)
                .wait()
                .expect("Unable to send R");
            System::current().stop();
            Ok(())
        }).into_future(),
    );
    sys.run();

    // Wait for either value comes first (error or ok)
    let res = rx.wait().nth(0);
    // Unwrap deeply nested value
    res.unwrap().unwrap()
}

lazy_static! {
    static ref CHANNEL_ADDRESS: Address = env::var("CHANNEL_ADDRESS")
        .expect("Unable to obtain channel manager contract address. Is $CHANNEL_ADDRESS set properly?")
        .parse()
        .expect("Unable to parse address passed in $CHANNEL_ADDRESS");
    static ref WEB3: Web3Client =
        make_web3().expect("Unable to create a valid transport for Web3 protocol");

    // // WEB3.
    static ref NETWORK_ID : u64 = block_on(WEB3.net_version())
    .unwrap().parse()
            .expect("Unable to obtain network ID");

    static ref ONE_ETH: Uint256 = "0xde0b6b3a7640000".parse().unwrap();

    // // Choose a seed key which is the first key returned by the network
    static ref SEED_ADDRESS : Address = block_on(WEB3
         .eth_accounts()
    )
         .expect("Unable to retrieve accounts")
         .into_iter()
         .nth(0)
         .expect("Unable to obtain first address from the test network");

    static ref BLOCK_NUMBER : Uint256 = block_on(WEB3.eth_block_number()).expect("Unable to convert block number");
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
        to: Some(key.to_public_key().unwrap()),
        gas: None,
        gas_price: Some(Uint256::from(0x1u64)),
        value: Some(ONE_ETH.clone() * Uint256::from(10u64)),
        data: None,
        nonce: None,
    };
    let _res = block_on(WEB3.eth_send_transaction(vec![tx_req])).unwrap();
    let res = block_on(WEB3.eth_get_balance(key.to_public_key().unwrap())).unwrap();
    println!("Balance {:?}", res);
    key
}

fn create_new_channel_fingerprint(
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

    let msg = create_new_channel_fingerprint_data(
        &*CHANNEL_ADDRESS,
        &address0,
        &address1,
        &balance0,
        &balance1,
        &expiration,
        &settling,
    );
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

    let msg = create_update_fingerprint_data(
        &*CHANNEL_ADDRESS,
        &channel_id,
        &sequence_number,
        &balance0,
        &balance1,
    );
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

    let msg = create_close_channel_fast_fingerprint_data(
        &*CHANNEL_ADDRESS,
        &channel_id,
        &sequence_number,
        &balance0,
        &balance1,
    );
    (secret0.sign_msg(&msg), secret1.sign_msg(&msg))
}

fn create_settling_fingerprint(key: &PrivateKey, channel_id: ChannelId) -> Signature {
    let mut msg = "startSettlingPeriod".as_bytes().to_vec();
    msg.extend(CHANNEL_ADDRESS.clone().as_bytes());
    msg.extend(channel_id.to_vec());
    key.sign_msg(&msg)
}

fn create_redraw_fingerprint(
    secret0: &PrivateKey,
    secret1: &PrivateKey,
    channel_id: ChannelId,
    sequence_number: Uint256,
    old_balance_a: Uint256,
    old_balance_b: Uint256,
    new_balance_a: Uint256,
    new_balance_b: Uint256,
    expiration: Uint256,
) -> (Signature, Signature) {
    let (secret0, secret1) = if secret0.to_public_key().unwrap() > secret1.to_public_key().unwrap()
    {
        (secret1, secret0)
    } else {
        (secret0, secret1)
    };

    let mut msg = create_redraw_fingerprint_data(
        &*CHANNEL_ADDRESS,
        &channel_id,
        &sequence_number,
        &old_balance_a,
        &old_balance_b,
        &new_balance_a,
        &new_balance_b,
        &expiration,
    );
    (secret0.sign_msg(&msg), secret1.sign_msg(&msg))
}

fn create_update_with_bounty_fingerprint(
    secret: &PrivateKey,
    channel_id: ChannelId,
    sequence_number: Uint256,
    balance0: Uint256,
    balance1: Uint256,
    signature0: Signature,
    signature1: Signature,
    bounty_amount: Uint256,
) -> Signature {
    let msg = create_update_with_bounty_fingerprint_data(
        &*CHANNEL_ADDRESS,
        &channel_id,
        &sequence_number,
        &balance0,
        &balance1,
        &signature0,
        &signature1,
        &bounty_amount,
    );
    secret.sign_msg(&msg)
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

    let contract: Box<PaymentContract> = Box::new(GuacContract::new());

    println!("Address {:?}", &*CHANNEL_ADDRESS);
    println!("Network ID {:?}", &*NETWORK_ID);

    // Bounty Hunter
    let bounty_hunter_balance: Uint256 = "1000000000000000000".parse().unwrap(); // 1ETH
    let bounty_hunter = make_seeded_key();
    *CRYPTO.secret_mut() = bounty_hunter.clone();
    let bounty_hunter_pk = bounty_hunter.to_public_key().unwrap();
    block_on(contract.quick_deposit(bounty_hunter_balance.clone())).unwrap();

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
    let gas_price = block_on(WEB3.eth_gas_price()).unwrap();

    // This has to be updated on every state update
    let mut channel_nonce = 0u32;

    let mut alice_balance: Uint256 = "1000000000000000000".parse().unwrap();
    let mut bob_balance: Uint256 = "0".parse().unwrap();
    let mut total_balance = alice_balance.clone() + bob_balance.clone();
    println!("total balance {}", total_balance);

    let expiration: Uint256 = (BLOCK_NUMBER.clone() + 100u64).into();
    let settling: Uint256 = 200u64.into();

    println!("Calling quickDeposit");
    block_on(contract.quick_deposit(alice_balance.clone())).unwrap();

    let (sig0, sig1) = create_new_channel_fingerprint(
        &alice,
        &bob,
        alice.to_public_key().unwrap(),
        bob.to_public_key().unwrap(),
        alice_balance.clone(),
        bob_balance.clone(),
        expiration.clone(),
        settling.clone(),
    );

    println!("Calling newChannel");
    let fut = block_on(contract.new_channel(
        alice.to_public_key().unwrap(),
        bob.to_public_key().unwrap(),
        alice_balance.clone().into(),
        bob_balance.clone().into(),
        sig0,
        sig1,
        expiration.clone().into(),
        settling.clone().into(),
    ));

    let channel_id = fut.unwrap();
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

    println!("Calling updateState");
    let fut = block_on(contract.update_state(
        channel_id,
        channel_nonce.clone().into(),
        balance0.clone(),
        balance1.clone(),
        sig_a.clone(),
        sig_b.clone(),
    ));

    fut.unwrap();

    //
    // Redraw
    //

    channel_nonce += 1;

    let old_balance0 = alice_balance.clone();
    let old_balance1 = bob_balance.clone();

    let op: Uint256 = "100000000000000000".parse().unwrap();
    assert!(op <= alice_balance);

    // alice_balance -= op.clone();
    // bob_balance += op.clone();

    // Alice deposits again
    println!("Calling quickDeposit again");
    block_on(contract.quick_deposit(op.clone())).unwrap();

    alice_balance += op.clone();

    assert_eq!(old_balance0.clone() + old_balance1.clone(), total_balance);

    total_balance += op.clone();
    assert_eq!(alice_balance.clone() + bob_balance.clone(), total_balance);

    let (old_balance0, old_balance1, new_balance0, new_balance1) =
        if alice.to_public_key().unwrap() > bob.to_public_key().unwrap() {
            (
                old_balance1,
                old_balance0,
                bob_balance.clone(),
                alice_balance.clone(),
            )
        } else {
            (
                old_balance0,
                old_balance1,
                alice_balance.clone(),
                bob_balance.clone(),
            )
        };

    println!(
        "{} {} {} {}",
        old_balance0, old_balance1, new_balance0, new_balance1
    );

    let expiration: Uint256 = (BLOCK_NUMBER.clone() + 100u64).into(); //expiration

    let (sig_a, sig_b) = create_redraw_fingerprint(
        &alice,
        &bob,
        channel_id,
        channel_nonce.clone().into(),
        old_balance0.clone(),
        old_balance1.clone(),
        new_balance0.clone(),
        new_balance1.clone(),
        expiration.clone(),
    );

    println!("Calling redraw");
    let fut = block_on(contract.redraw(
        channel_id,
        channel_nonce.clone().into(),
        old_balance0.clone(),
        old_balance1.clone(),
        new_balance0.clone(),
        new_balance1.clone(),
        expiration.clone(),
        sig_a,
        sig_b,
    ));
    fut.unwrap();

    channel_nonce += 1;

    let sig = create_settling_fingerprint(&alice, channel_id);
    println!("Calling start settling period");
    block_on(contract.start_settling_period(channel_id, sig)).unwrap();

    channel_nonce += 1;
    println!("Calling updateStateWithBounty");

    let (balance0, balance1) = if alice.to_public_key().unwrap() > bob.to_public_key().unwrap() {
        (bob_balance.clone(), alice_balance.clone())
    } else {
        (alice_balance.clone(), bob_balance.clone())
    };
    let (sig_a, sig_b) = create_update_fingerprint(
        &alice,
        &bob,
        channel_id.clone(),
        channel_nonce.clone().into(),
        balance0.clone(),
        balance1.clone(),
    );
    let bounty: Uint256 = "1234".parse().unwrap();

    *CRYPTO.secret_mut() = bounty_hunter.clone();

    let fut = block_on(contract.update_state_with_bounty(
        channel_id,
        channel_nonce.clone().into(),
        balance0.clone(),
        balance1.clone(),
        sig_a.clone(),
        sig_b.clone(),
        bounty.clone(), // bounty
        create_update_with_bounty_fingerprint(
            &bounty_hunter,
            channel_id,
            channel_nonce.clone().into(),
            balance0.clone(),
            balance1.clone(),
            sig_a.clone(),
            sig_b.clone(),
            bounty.clone(),
        ),
    ));

    fut.unwrap();

    *CRYPTO.secret_mut() = alice.clone();
    channel_nonce += 1;

    let (balance0, balance1) = if alice.to_public_key().unwrap() > bob.to_public_key().unwrap() {
        (bob_balance.clone(), alice_balance.clone())
    } else {
        (alice_balance.clone(), bob_balance.clone())
    };
    let (sig_a, sig_b) = create_close_fingerprint(
        &alice,
        &bob,
        channel_id.clone(),
        channel_nonce.clone().into(),
        balance0.clone(),
        balance1.clone(),
    );

    println!("Calling closeChannelFast");
    let fut = block_on(contract.close_channel_fast(
        channel_id,
        channel_nonce.clone().into(),
        balance0.clone(),
        balance1.clone(),
        sig_a,
        sig_b,
    ));
    fut.unwrap();

    println!("Calling withdraw");

    block_on(contract.withdraw(alice_balance.clone())).unwrap();
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

    assert!(block_on(CRYPTO.get_network_id()).unwrap().len() != 0);
    assert_eq!(block_on(CRYPTO.get_nonce()).unwrap(), Uint256::from(0u64));
    assert_ne!(
        block_on(CRYPTO.get_gas_price()).unwrap(),
        Uint256::from(0u64)
    );
    assert_eq!(
        block_on(CRYPTO.get_network_balance()).unwrap(),
        Uint256::from(0u64)
    );
}
