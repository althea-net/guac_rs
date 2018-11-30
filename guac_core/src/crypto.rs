use clarity::abi::derive_signature;
use clarity::utils::bytes_to_hex_str;
use clarity::Transaction;
use clarity::{Address, PrivateKey, Signature};
use failure::Error;
use multihash::{encode, Hash};

use owning_ref::{RwLockReadGuardRef, RwLockWriteGuardRefMut};
use sha3::{Digest, Keccak256};
use std::sync::{Arc, RwLock};

use error::GuacError;
use futures::future::ok;
use futures::Future;
use futures::IntoFuture;
use futures::Stream;
use num256::Uint256;
use std::time;
use web3::client::{Web3, Web3Client};
use web3::types::{Log, NewFilter};

/// A global object which is responsible for managing all crypo related things.
lazy_static! {
    pub static ref CRYPTO: Arc<RwLock<Crypto>> = Arc::new(RwLock::new(Crypto::new()));
}

pub struct Config {
    /// Network address
    pub address: String,
    /// Address for the contract
    pub contract: Address,
    /// Private key
    pub secret: PrivateKey,
}

pub struct Crypto {
    pub secret: PrivateKey,

    /// This is a cached local balance
    pub balance: Uint256,

    // Handle to a Web3 instance
    pub web3: Option<Web3Client>,

    /// Contract address
    pub contract: Address,
}

pub enum Action {
    /// Sends a "traditional" ETH transfer
    To(Address),
    /// Does a contract call with provided ddata
    Call(Vec<u8>),
}

pub trait CryptoService {
    fn init(&self, config: &Config) -> Result<(), Error>;
    fn own_eth_addr(&self) -> Address;
    fn secret(&self) -> PrivateKey;
    fn secret_mut<'ret, 'me: 'ret>(&'me self) -> RwLockWriteGuardRefMut<'ret, Crypto, PrivateKey>;
    fn get_balance_mut<'ret, 'me: 'ret>(&'me self)
        -> RwLockWriteGuardRefMut<'ret, Crypto, Uint256>;
    /// Access local balance without querying network.
    ///
    /// This is different from get_network_balance, where an actual
    /// network call is made to retrieve up to date balance. This method
    /// should be preferred over querying the network.
    fn get_balance(&self) -> Uint256;
    fn eth_sign(&self, data: &[u8]) -> Signature;
    fn hash_bytes(&self, x: &[&[u8]]) -> Uint256;
    fn verify(_fingerprint: &Uint256, _signature: &Signature, _address: Address) -> bool;
    fn web3<'ret, 'me: 'ret>(&'me self) -> RwLockReadGuardRef<'ret, Crypto, Web3Client>;

    // Async stuff
    fn get_network_id(&self) -> Box<Future<Item = String, Error = Error>>;
    fn get_nonce(&self) -> Box<Future<Item = Uint256, Error = Error>>;
    fn get_gas_price(&self) -> Box<Future<Item = Uint256, Error = Error>>;
    /// Queries the network for current balance. This is different
    /// from get_balance which keeps track of local balance to save
    /// up on network calls.
    ///
    /// This function shouldn't be called every time. Ideally it should be
    /// called once when initializing private key, or periodically to synchronise
    /// local and network balance.
    fn get_network_balance(&self) -> Box<Future<Item = Uint256, Error = Error>>;
    /// Waits for an event on the network using the event name.
    ///
    /// * `event` - Event signature
    /// * `topic` - First topic to filter out
    fn wait_for_event(
        &self,
        event: &str,
        topic1: Option<Vec<[u8; 32]>>,
        topic2: Option<Vec<[u8; 32]>>,
    ) -> Box<Future<Item = Log, Error = Error>>;
    /// Broadcast a transaction on the network.
    ///
    /// * `action` - Defines a type of transaction
    /// * `value` - How much wei to send
    fn broadcast_transaction(
        &self,
        action: Action,
        value: Uint256,
    ) -> Box<Future<Item = Uint256, Error = Error>>;
}

impl Crypto {
    fn new() -> Crypto {
        Crypto {
            secret: "1010101010101010101010101010101010101010101010101010101010101010"
                .parse()
                .unwrap(),
            balance: 1_000_000_000_000u64.into(),
            // TODO: Proper connecting
            web3: None,
            contract: Address::default(),
        }
    }
}

fn bytes_to_data(s: &[u8]) -> String {
    let mut foo = "0x".to_string();
    foo.push_str(&bytes_to_hex_str(&s));
    foo
}

impl CryptoService for Arc<RwLock<Crypto>> {
    fn init(&self, config: &Config) -> Result<(), Error> {
        let mut service = self.write().unwrap();
        service.web3 = Some(Web3Client::new(&config.address));
        service.contract = config.contract.clone();
        service.secret = config.secret.clone();
        Ok(())
    }
    fn own_eth_addr(&self) -> Address {
        self.read()
            .unwrap()
            .secret
            .to_public_key()
            .expect("Unable to obtain public key")
    }
    fn secret(&self) -> PrivateKey {
        self.read().unwrap().secret.clone()
    }
    fn secret_mut<'ret, 'me: 'ret>(&'me self) -> RwLockWriteGuardRefMut<'ret, Crypto, PrivateKey> {
        RwLockWriteGuardRefMut::new(self.write().unwrap()).map_mut(|c| &mut c.secret)
    }
    fn get_balance_mut<'ret, 'me: 'ret>(
        &'me self,
    ) -> RwLockWriteGuardRefMut<'ret, Crypto, Uint256> {
        RwLockWriteGuardRefMut::new(self.write().unwrap()).map_mut(|c| &mut c.balance)
    }
    fn get_balance(&self) -> Uint256 {
        self.read().unwrap().balance.clone()
    }
    fn eth_sign(&self, data: &[u8]) -> Signature {
        self.read().unwrap().secret.sign_hash(data)
    }
    fn hash_bytes(&self, x: &[&[u8]]) -> Uint256 {
        let mut hasher = Keccak256::new();
        for buffer in x {
            hasher.input(*buffer)
        }
        let bytes = hasher.result();
        Uint256::from_bytes_be(&bytes)
    }
    fn verify(_fingerprint: &Uint256, _signature: &Signature, _address: Address) -> bool {
        unimplemented!("verify")
    }
    fn web3<'ret, 'me: 'ret>(&'me self) -> RwLockReadGuardRef<'ret, Crypto, Web3Client> {
        RwLockReadGuardRef::new(self.read().unwrap()).map(|c| {
            // To use web3 you need to call CRYPTO.init first.
            assert!(c.web3.is_some(), "Web3 connection is not initialized.");
            c.web3.as_ref().unwrap()
        })
    }
    fn get_network_id(&self) -> Box<Future<Item = String, Error = Error>> {
        self.web3().net_version()
    }
    fn get_nonce(&self) -> Box<Future<Item = Uint256, Error = Error>> {
        self.web3()
            .eth_get_transaction_count(self.own_eth_addr().clone())
    }
    fn get_gas_price(&self) -> Box<Future<Item = Uint256, Error = Error>> {
        self.web3().eth_gas_price()
    }
    fn get_network_balance(&self) -> Box<Future<Item = Uint256, Error = Error>> {
        self.web3().eth_get_balance(self.own_eth_addr().clone())
    }

    fn wait_for_event(
        &self,
        event: &str,
        topic1: Option<Vec<[u8; 32]>>,
        topic2: Option<Vec<[u8; 32]>>,
    ) -> Box<Future<Item = Log, Error = Error>> {
        // Build a filter with specified topics
        let mut new_filter = NewFilter::default();
        new_filter.address = vec![self.read().unwrap().contract.clone()];
        new_filter.topics = Some(vec![
            Some(vec![Some(bytes_to_data(&derive_signature(event)))]),
            topic1.map(|v| v.into_iter().map(|val| Some(bytes_to_data(&val))).collect()),
            topic2.map(|v| v.into_iter().map(|val| Some(bytes_to_data(&val))).collect()),
        ]);

        // let filter = FilterBuilder::default()
        //     .address(
        //         // Convert contract address into eth-types
        //         vec![self.read().unwrap().contract.as_bytes().into()],
        //     ).topics(
        //         Some(vec![derive_signature(event).into()]),
        //         // This is a first, optional topic to filter. If specified it will be converted
        //         // into a vector of values, otherwise a None.
        //         topic1.map(|v| v.iter().map(|&val| val.into()).collect()),
        //         topic2.map(|v| v.iter().map(|&val| val.into()).collect()),
        //         None,
        //     ).build();
        Box::new(
            CRYPTO
                .web3()
                .eth_new_filter(vec![new_filter])
                .from_err()
                .and_then(move |filter_id| {
                    CRYPTO
                        .web3()
                        .eth_get_filter_changes(filter_id)
                        .into_future()
                        .map(|(head, _tail)| head)
                        .map_err(|(e, _)| e)
                }).from_err()
                .map(move |maybe_log| maybe_log.expect("Expected log data but None found"))
                .into_future(),
        )

        // Box::new(
        //     self.web3()
        //         .eth_filter()
        //         .create_logs_filter(filter)
        //         .then(|filter| {
        //             filter
        //                 .unwrap()
        //                 .stream(time::Duration::from_secs(0))
        //                 .into_future()
        //                 .map(|(head, _tail)| {
        //                     // Throw away rest of the stream
        //                     head
        //                 })
        //         }).map_err(|(e, _)| e)
        //         .map_err(GuacError::from)
        //         .from_err()
        //         .map(|maybe_log| maybe_log.expect("Expected log data but None found"))
        //         .into_future(),
        // )
    }

    fn broadcast_transaction(
        &self,
        action: Action,
        value: Uint256,
    ) -> Box<Future<Item = Uint256, Error = Error>> {
        // We're not relying on web3 signing functionality. Here we're do the signing ourselves.
        let props = self
            .get_network_id()
            .join3(self.get_gas_price(), self.get_nonce());
        // let instance = self.read().unwrap();
        let contract = self.read().unwrap().contract.clone();
        let secret = self.read().unwrap().secret.clone();
        // let web3 = self.web3().clone();

        Box::new(
            props
                .and_then(move |(network_id, gas_price, nonce)| {
                    let transaction = match action {
                        Action::To(address) => Transaction {
                            to: address.clone(),
                            nonce: nonce,
                            gas_price: gas_price.into(),
                            gas_limit: 6721975u32.into(),
                            value: value,
                            data: Vec::new(),
                            signature: None,
                        },
                        Action::Call(data) => Transaction {
                            to: contract.clone(),
                            nonce: nonce,
                            gas_price: gas_price.into(),
                            gas_limit: 6721975u32.into(),
                            value: value,
                            data: data,
                            signature: None,
                        },
                    };

                    let transaction = transaction.sign(&secret, Some(network_id.parse().unwrap()));

                    CRYPTO
                        .web3()
                        .eth_send_raw_transaction(transaction.to_bytes().unwrap())
                    // .into_future()
                    // .map_err(GuacError::from)
                    // .and_then(|tx| ok(format!("0x{:x}", tx).parse().unwrap()))
                    // .from_err()
                }).into_future(),
        )
    }
}

#[test]
fn create() {
    &*CRYPTO;
}
