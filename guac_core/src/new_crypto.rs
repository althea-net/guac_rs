use clarity::{Address, PrivateKey, Signature};
use num256::uint256::Uint256;
use sha3::{Digest, Keccak256};

#[derive(Default)]
pub struct Crypto {
    pub contract_address: Address,
    pub own_address: Address,
    pub secret: PrivateKey,
}

impl Crypto {
    pub fn eth_sign(&self, data: &[u8]) -> Signature {
        self.secret.sign_hash(data)
    }
}

pub fn hash_bytes(x: &[&[u8]]) -> Uint256 {
    let mut hasher = Keccak256::new();
    for buffer in x {
        hasher.input(*buffer)
    }
    let bytes = hasher.result();
    Uint256::from_bytes_be(&bytes)
}
