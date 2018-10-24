use clarity::abi::derive_signature;
use clarity::Transaction;
use clarity::{Address, PrivateKey, Signature};
use failure::Error;
use multihash::{encode, Hash};
use network::Web3Handle;

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
use web3::types::{Bytes, FilterBuilder, Log};

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
    pub web3: Option<Web3Handle>,

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
    fn web3<'ret, 'me: 'ret>(&'me self) -> RwLockReadGuardRef<'ret, Crypto, Web3Handle>;

    // Async stuff
    fn get_network_id(&self) -> Box<Future<Item = u64, Error = Error>>;
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
    fn wait_for_event(&self, event: &str) -> Box<Future<Item = Log, Error = Error>>;
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

impl CryptoService for Arc<RwLock<Crypto>> {
    fn init(&self, config: &Config) -> Result<(), Error> {
        let mut service = self.write().unwrap();
        service.web3 = Some(Web3Handle::new(&config.address)?);
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
    fn web3<'ret, 'me: 'ret>(&'me self) -> RwLockReadGuardRef<'ret, Crypto, Web3Handle> {
        RwLockReadGuardRef::new(self.read().unwrap()).map(|c| {
            // To use web3 you need to call CRYPTO.init first.
            assert!(c.web3.is_some(), "Web3 connection is not initialized.");
            c.web3.as_ref().unwrap()
        })
    }
    fn get_network_id(&self) -> Box<Future<Item = u64, Error = Error>> {
        Box::new(
            self.web3()
                .net()
                .version()
                .into_future()
                .map_err(GuacError::from)
                .from_err()
                .map(|value| {
                    // According to https://github.com/ethereum/wiki/wiki/JSON-RPC#net_version
                    // server sends network id as a string.
                    value
                        .parse()
                        .expect("Network was expected to return a valid network ID")
                }),
        )
    }
    fn get_nonce(&self) -> Box<Future<Item = Uint256, Error = Error>> {
        Box::new(
            self.web3()
                .eth()
                .transaction_count(self.own_eth_addr().to_string().parse().unwrap(), None)
                .into_future()
                .map_err(GuacError::from)
                .from_err()
                // Ugly conversion routine from ethereum-types -> clarity
                .map(|value| value.to_string().parse().unwrap()),
        )
    }
    fn get_gas_price(&self) -> Box<Future<Item = Uint256, Error = Error>> {
        Box::new(
            self.web3()
                .eth()
                .gas_price()
                .into_future()
                .map_err(GuacError::from)
                .from_err()
                // Ugly conversion routine from ethereum-types -> clarity
                .map(|value| value.to_string().parse().unwrap()),
        )
    }
    fn get_network_balance(&self) -> Box<Future<Item = Uint256, Error = Error>> {
        Box::new(
            self.web3()
                .eth()
                .balance(self.own_eth_addr().to_string().parse().unwrap(), None)
                .into_future()
                .map_err(GuacError::from)
                .from_err()
                // Ugly conversion routine from ethereum-types -> clarity
                .map(|value| value.to_string().parse().unwrap()),
        )
    }

    fn wait_for_event(&self, event: &str) -> Box<Future<Item = Log, Error = Error>> {
        // Build a filter
        let filter = FilterBuilder::default()
            .address(vec![
                // Convert contract address into eth-types
                self.read().unwrap().contract.to_string().parse().unwrap(),
            ]).topics(Some(vec![derive_signature(event).into()]), None, None, None)
            .build();

        Box::new(
            self.web3()
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
                }).map_err(|(e, _)| e)
                .map_err(GuacError::from)
                .from_err()
                .map(|maybe_log| maybe_log.expect("Expected log data but None found"))
                .into_future(),
        )
    }

    fn broadcast_transaction(
        &self,
        action: Action,
        value: Uint256,
    ) -> Box<Future<Item = Uint256, Error = Error>> {
        // We're not relying on web3 signing functionality. Here we're do the signing ourselves.
        let props = self
            .get_network_id()
            .join3(self.get_network_id(), self.get_nonce());
        // let instance = self.read().unwrap();
        let contract = self.read().unwrap().contract.clone();
        let secret = self.read().unwrap().secret.clone();
        let web3 = self.web3().clone();

        Box::new(
            props
                .and_then(move |(network_id, gas_price, nonce)| {
                    // ok(Uint256::from(0u64))
                    let transaction = match action {
                        Action::To(address) => {
                            Transaction {
                                to: address.clone(),
                                // action: Action::Call(Address::default()),
                                // TODO: Get nonce from eth full node
                                nonce: nonce,
                                // TODO: set this semi automatically
                                gas_price: gas_price.into(),
                                // TODO: find out how much gas this contract acutally takes
                                gas_limit: 6721975u32.into(),
                                value: value,
                                data: Vec::new(),
                                signature: None,
                            }
                            //.sign(&self.secret(), )
                        }
                        Action::Call(data) => {
                            Transaction {
                                to: contract.clone(),
                                // action: Action::Call(Address::default()),
                                // TODO: Get nonce from eth full node
                                nonce: nonce,
                                // TODO: set this semi automatically
                                gas_price: gas_price.into(),
                                // TODO: find out how much gas this contract acutally takes
                                gas_limit: 6721975u32.into(),
                                value: value,
                                data: data,
                                signature: None,
                            }
                        }
                    };

                    let transaction = transaction.sign(&secret, Some(network_id));

                    web3.eth()
                        .send_raw_transaction(Bytes::from(transaction.to_bytes().unwrap()))
                        .into_future()
                        .map_err(GuacError::from)
                        .and_then(|tx| ok(tx.to_string().parse().unwrap()))
                        .from_err()
                }).into_future(),
        )
    }
}

#[test]
fn create() {
    &*CRYPTO;
}
