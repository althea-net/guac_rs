use althea_types::{Bytes32, EthAddress, EthSignature};
use ethkey::{sign, Generator, KeyPair, Message, Random, Secret, Signature};
use multihash::{encode, Hash};
use num256::Uint256;

use owning_ref::RwLockWriteGuardRefMut;
use std::sync::{Arc, RwLock};

/// A global object which stores per node crypto state
lazy_static! {
    pub static ref CRYPTO: Arc<RwLock<Crypto>> = Arc::new(RwLock::new(Crypto::new()));
}

pub struct Crypto {
    pub key_pair: KeyPair,
    pub balance: Uint256,
}

pub trait CryptoService {
    fn own_eth_addr(&self) -> EthAddress;
    fn own_secret(&self) -> Secret;
    fn get_balance_mut<'ret, 'me: 'ret>(&'me self)
        -> RwLockWriteGuardRefMut<'ret, Crypto, Uint256>;
    fn get_balance(&self) -> Uint256;
    fn eth_sign(&self, data: &[u8]) -> Signature;
    fn hash_bytes(&self, x: &[&[u8]]) -> Bytes32;
    fn verify(_fingerprint: &Bytes32, _signature: &EthSignature, _address: EthAddress) -> bool;
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
    fn own_eth_addr(&self) -> EthAddress {
        self.read().unwrap().key_pair.address()
    }
    fn own_secret(&self) -> Secret {
        self.read().unwrap().key_pair.secret().clone()
    }
    fn get_balance_mut<'ret, 'me: 'ret>(
        &'me self,
    ) -> RwLockWriteGuardRefMut<'ret, Crypto, Uint256> {
        RwLockWriteGuardRefMut::new(self.write().unwrap()).map_mut(|c| &mut c.balance)
    }
    fn get_balance(&self) -> Uint256 {
        self.read().unwrap().balance
    }
    fn eth_sign(&self, data: &[u8]) -> Signature {
        let fingerprint = encode(Hash::Keccak256, &data).unwrap();
        let msg = Message::from_slice(&fingerprint[2..]);
        let sig = sign(&self.read().unwrap().key_pair.secret(), &msg).unwrap();
        sig
    }
    fn hash_bytes(&self, _x: &[&[u8]]) -> Bytes32 {
        0.into()
    }
    fn verify(_fingerprint: &Bytes32, _signature: &EthSignature, _address: EthAddress) -> bool {
        true
    }
}
