use clarity::{Address, PrivateKey, Signature};
use num256::uint256::Uint256;
use sha3::{Digest, Keccak256};

#[derive(Default)]
pub struct Crypto {
    pub full_node_url: String,
    pub contract_address: Address,
    pub secret: PrivateKey,
}

impl Crypto {
    pub fn own_eth_addr(&self) -> Address {
        self.secret
            .to_public_key()
            .expect("Unable to obtain public key")
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

pub fn eth_sign(secret: PrivateKey, data: &[u8]) -> Signature {
    secret.sign_hash(data)
}
