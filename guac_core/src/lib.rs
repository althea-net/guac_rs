extern crate env_logger;
extern crate futures;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate base64;
#[macro_use]
extern crate failure;
extern crate clarity;
extern crate serde_json;
extern crate tiny_keccak;
extern crate uuid;
#[macro_use]
extern crate lazy_static;
extern crate hex;
extern crate multihash;
extern crate owning_ref;
extern crate qutex;
#[macro_use]
extern crate log;
#[cfg(test)]
extern crate actix;
extern crate actix_web;
extern crate futures_timer;
#[cfg(test)]
extern crate mockito;
extern crate num;
extern crate num256;
extern crate sha3;
extern crate tokio;

// Traits
pub mod payment_contract;

#[macro_use]
pub mod channel_client;
pub mod error;
pub mod new_crypto;
pub mod storage;

// pub mod transport_protocol;
// pub mod web3;

pub use self::channel_client::channel_manager::BlockchainApi;
pub use self::channel_client::channel_manager::CounterpartyApi;
pub use self::channel_client::channel_manager::Guac;
pub use self::channel_client::channel_manager::UserApi;
pub use self::channel_client::types;
pub use self::channel_client::types::GuacError;
pub use self::new_crypto::Crypto;
pub use self::storage::Storage;
