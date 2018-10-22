use clarity::{Address, PrivateKey, Signature};
use multihash::{encode, Hash};
use network::Web3Handle;

use owning_ref::{RwLockReadGuardRef, RwLockWriteGuardRefMut};
use sha3::{Digest, Keccak256};
use std::sync::{Arc, RwLock};

use num256::Uint256;

/// A global object which stores per node crypto state
lazy_static! {
    pub static ref CRYPTO: Arc<RwLock<Crypto>> = Arc::new(RwLock::new(Crypto::new()));
}

pub struct Crypto {
    pub secret: PrivateKey,

    /// This is a local balance which is just a hack for testing things
    pub balance: Uint256,

    // Handle to a Web3 instance
    pub web3: Web3Handle,
}

pub trait CryptoService {
    fn own_eth_addr(&self) -> Address;
    fn secret(&self) -> PrivateKey;
    fn secret_mut<'ret, 'me: 'ret>(&'me self) -> RwLockWriteGuardRefMut<'ret, Crypto, PrivateKey>;
    fn get_balance_mut<'ret, 'me: 'ret>(&'me self)
        -> RwLockWriteGuardRefMut<'ret, Crypto, Uint256>;
    fn get_balance(&self) -> Uint256;
    fn eth_sign(&self, data: &[u8]) -> Signature;
    fn hash_bytes(&self, x: &[&[u8]]) -> Uint256;
    fn verify(_fingerprint: &Uint256, _signature: &Signature, _address: Address) -> bool;
    fn web3<'ret, 'me: 'ret>(&'me self) -> RwLockReadGuardRef<'ret, Crypto, Web3Handle>;
}

impl Crypto {
    fn new() -> Crypto {
        Crypto {
            secret: "1010101010101010101010101010101010101010101010101010101010101010"
                .parse()
                .unwrap(),
            balance: 1_000_000_000_000u64.into(),
            // TODO: Proper connecting
            web3: Web3Handle::new("http://localhost:8545").unwrap(),
        }
    }
}

impl CryptoService for Arc<RwLock<Crypto>> {
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
        RwLockReadGuardRef::new(self.read().unwrap()).map(|c| &c.web3)
    }
}
