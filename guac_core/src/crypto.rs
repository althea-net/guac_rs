// use althea_types::{U256, Address, Signature};
use ethereum_types::{Address, U256};
use ethkey::{sign, Generator, KeyPair, Message, Random, Secret, Signature};
use multihash::{encode, Hash};

use owning_ref::RwLockWriteGuardRefMut;
use std::sync::{Arc, RwLock};

/// A global object which stores per node crypto state
lazy_static! {
    pub static ref CRYPTO: Arc<RwLock<Crypto>> = Arc::new(RwLock::new(Crypto::new()));
}

pub struct Crypto {
    pub key_pair: KeyPair,

    /// This is a local balance which is just a hack for testing things
    pub balance: U256,
}

pub trait CryptoService {
    fn own_eth_addr(&self) -> Address;
    fn secret(&self) -> Secret;
    fn get_balance_mut<'ret, 'me: 'ret>(&'me self)
        -> RwLockWriteGuardRefMut<'ret, Crypto, U256>;
    fn get_balance(&self) -> U256;
    fn eth_sign(&self, data: &[u8]) -> Signature;
    fn hash_bytes(&self, x: &[&[u8]]) -> U256;
    fn verify(_fingerprint: &U256, _signature: &Signature, _address: Address) -> bool;
}

impl Crypto {
    fn new() -> Crypto {
        Crypto {
            key_pair: Random::generate(&mut Random {}).unwrap(),
            balance: 1_000_000_000_000u64.into(),
        }
    }
}

impl CryptoService for Arc<RwLock<Crypto>> {
    fn own_eth_addr(&self) -> Address {
        self.read().unwrap().key_pair.address()
    }
    fn secret(&self) -> Secret {
        self.read().unwrap().key_pair.secret().clone()
    }
    fn get_balance_mut<'ret, 'me: 'ret>(
        &'me self,
    ) -> RwLockWriteGuardRefMut<'ret, Crypto, U256> {
        RwLockWriteGuardRefMut::new(self.write().unwrap()).map_mut(|c| &mut c.balance)
    }
    fn get_balance(&self) -> U256 {
        self.read().unwrap().balance
    }
    fn eth_sign(&self, data: &[u8]) -> Signature {
        let fingerprint = encode(Hash::Keccak256, &data).unwrap();
        let msg = Message::from_slice(&fingerprint[2..]);
        let sig = sign(&self.read().unwrap().key_pair.secret(), &msg).unwrap();
        sig
    }
    fn hash_bytes(&self, _x: &[&[u8]]) -> U256 {
        0.into()
    }
    fn verify(_fingerprint: &U256, _signature: &Signature, _address: Address) -> bool {
        true
    }
}
