use clarity::{Address, BigEndianInt, PrivateKey, Signature};
use multihash::{encode, Hash};

use owning_ref::RwLockWriteGuardRefMut;
use sha3::{Digest, Keccak256};
use std::sync::{Arc, RwLock};

/// A global object which stores per node crypto state
lazy_static! {
    pub static ref CRYPTO: Arc<RwLock<Crypto>> = Arc::new(RwLock::new(Crypto::new()));
}

pub struct Crypto {
    pub secret: PrivateKey,

    /// This is a local balance which is just a hack for testing things
    pub balance: BigEndianInt,
}

pub trait CryptoService {
    fn own_eth_addr(&self) -> Address;
    fn secret(&self) -> PrivateKey;
    fn get_balance_mut<'ret, 'me: 'ret>(
        &'me self,
    ) -> RwLockWriteGuardRefMut<'ret, Crypto, BigEndianInt>;
    fn get_balance(&self) -> BigEndianInt;
    fn eth_sign(&self, data: &[u8]) -> Signature;
    fn hash_bytes(&self, x: &[&[u8]]) -> BigEndianInt;
    fn verify(_fingerprint: &BigEndianInt, _signature: &Signature, _address: Address) -> bool;
}

impl Crypto {
    fn new() -> Crypto {
        Crypto {
            secret: "1010101010101010101010101010101010101010101010101010101010101010"
                .parse()
                .unwrap(),
            balance: 1_000_000_000_000u64.into(),
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
    fn get_balance_mut<'ret, 'me: 'ret>(
        &'me self,
    ) -> RwLockWriteGuardRefMut<'ret, Crypto, BigEndianInt> {
        RwLockWriteGuardRefMut::new(self.write().unwrap()).map_mut(|c| &mut c.balance)
    }
    fn get_balance(&self) -> BigEndianInt {
        self.read().unwrap().balance.clone()
    }
    fn eth_sign(&self, data: &[u8]) -> Signature {
        self.read().unwrap().secret.sign_hash(data)
    }
    fn hash_bytes(&self, x: &[&[u8]]) -> BigEndianInt {
        let mut hasher = Keccak256::new();
        for buffer in x {
            hasher.input(*buffer)
        }
        let bytes = hasher.result();
        BigEndianInt::from_bytes_be(&bytes)
    }
    fn verify(_fingerprint: &BigEndianInt, _signature: &Signature, _address: Address) -> bool {
        unimplemented!("verify")
    }
}
