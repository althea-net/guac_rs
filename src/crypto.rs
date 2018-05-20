use althea_types::{Bytes32, EthAddress, EthPrivateKey, EthSignature};
use ethkey::{sign, Generator, KeyPair, Message, Random};
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
    pub fn eth_sign(&self, data: &[u8]) -> EthSignature {
        let fingerprint = encode(Hash::Keccak256, &data).unwrap();
        let msg = Message::from_slice(&fingerprint[2..]);
        let sig = sign(&self.key_pair.secret(), &msg).unwrap();
        EthSignature(sig.into())
    }
    pub fn hash_bytes(&self, x: &[&[u8]]) -> Bytes32 {
        Bytes32([0; 32])
    }
    pub fn verify(_fingerprint: &Bytes32, _signature: &EthSignature, _address: EthAddress) -> bool {
        true
    }
}
