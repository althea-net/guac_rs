use althea_types::{Bytes32, EthAddress, EthPrivateKey, EthSignature};
use ethkey::{sign, Generator, KeyPair, Message, Random, Signature};
use failure::Error;
use multihash::{encode, Hash};
use std::collections::HashMap;

lazy_static! {
    pub static ref CRYPTO: Box<Crypto> = Box::new(Crypto::new());
}

#[derive(Debug, Fail)]
enum CryptoError {
    #[fail(display = "EthAddress not found in keystore.")]
    EthAddressNotFound {},
}

pub struct Crypto {
    pub key_pair: KeyPair,
}

impl Crypto {
    pub fn new() -> Crypto {
        Crypto {
            key_pair: Random::generate(&mut Random {}).unwrap(),
        }
    }
    pub fn own_eth_addr(&self) -> EthAddress {
        self.key_pair.address()
    }
    pub fn eth_sign(&self, data: &[u8]) -> Signature {
        let fingerprint = encode(Hash::Keccak256, &data).unwrap();
        let msg = Message::from_slice(&fingerprint[2..]);
        let sig = sign(&self.key_pair.secret(), &msg).unwrap();
        sig
    }
    pub fn hash_bytes(&self, x: &[&[u8]]) -> Bytes32 {
        0.into()
    }
    pub fn verify(_fingerprint: &Bytes32, _signature: &EthSignature, _address: EthAddress) -> bool {
        true
    }
}
