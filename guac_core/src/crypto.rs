// use althea_types::{BigEndianInt, Address, Signature};
use clarity::{Address, BigEndianInt};
// use ethkey::{sign, Generator, KeyPair, Message, Random, Secret, Signature};
use clarity::{PrivateKey, Signature, Transaction};
use multihash::{encode, Hash};

use owning_ref::RwLockWriteGuardRefMut;
use std::sync::{Arc, RwLock};

/// A global object which stores per node crypto state
lazy_static! {
    pub static ref CRYPTO: Crypto = Crypto::new();
}

pub struct Crypto {
    pub secret: PrivateKey,

    /// This is a local balance which is just a hack for testing things
    pub balance: BigEndianInt,
}

pub trait CryptoService {
    fn own_eth_addr(&self) -> Address;
    fn secret(&self) -> &PrivateKey;
    fn get_balance_mut<'ret, 'me: 'ret>(&'me mut self) -> &'ret mut BigEndianInt;
    fn get_balance(&self) -> &BigEndianInt;
    fn eth_sign(&self, data: &[u8]) -> Signature;
    fn hash_bytes(&self, x: &[&[u8]]) -> BigEndianInt;
    fn verify(_fingerprint: &BigEndianInt, _signature: &Signature, _address: Address) -> bool;
}

impl Crypto {
    fn new() -> Crypto {
        Crypto {
            secret: PrivateKey::new(),
            balance: 1_000_000_000_000u64.into(),
        }
    }
}

impl CryptoService for Crypto {
    fn own_eth_addr(&self) -> Address {
        self.secret.to_public_key().unwrap()
        // self.read().unwrap().secret.to_public_key().unwrap()
    }
    fn secret(&self) -> &PrivateKey {
        &self.secret
    }
    fn get_balance_mut<'ret, 'me: 'ret>(&'me mut self) -> &'ret mut BigEndianInt {
        &mut self.balance
    }
    fn get_balance(&self) -> &BigEndianInt {
        &self.balance
    }
    fn eth_sign(&self, data: &[u8]) -> Signature {
        /*
        let fingerprint = encode(Hash::Keccak256, &data).unwrap();
        let msg = Message::from_slice(&fingerprint[2..]);
        let sig = sign(&self.read().unwrap().key_pair.secret(), &msg).unwrap();
        sig*/
        Signature::new(0u8.into(), 0u8.into(), 0u8.into())
    }
    fn hash_bytes(&self, _x: &[&[u8]]) -> BigEndianInt {
        0u64.into()
    }
    fn verify(_fingerprint: &BigEndianInt, _signature: &Signature, _address: Address) -> bool {
        true
    }
}
