use althea_types::{Bytes32, EthAddress, EthPrivateKey, EthSignature};
use failure::Error;
use std::collections::HashMap;

#[derive(Debug, Fail)]
enum CryptoError {
    #[fail(display = "EthAddress not found in keystore.")]
    EthAddressNotFound {},
}

pub struct Crypto {
    keystore: HashMap<EthAddress, EthPrivateKey>,
}

impl Crypto {
    pub fn new() -> Crypto {
        Crypto {
            keystore: HashMap::new(),
        }
    }
    pub fn sign(&self, _address: &EthAddress, _hash: &Bytes32) -> Result<EthSignature, Error> {
        match self.keystore.get(_address) {
            None => Err(Error::from(CryptoError::EthAddressNotFound {})),
            Some(_pk) => Ok(EthSignature([0; 65])),
        }
    }
    pub fn hash(_input: Vec<Bytes32>) -> Bytes32 {
        Bytes32([0; 32])
    }
    pub fn verify(_fingerprint: &Bytes32, _signature: &EthSignature, _address: EthAddress) -> bool {
        true
    }
}
