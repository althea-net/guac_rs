extern crate actix;
extern crate actix_web;
#[macro_use]
extern crate diesel;
extern crate env_logger;
extern crate futures;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate althea_types;
extern crate base64;
#[macro_use]
extern crate failure;
extern crate num256;
extern crate serde_json;
extern crate tiny_keccak;
extern crate uuid;
extern crate web3;
#[macro_use]
extern crate lazy_static;

mod crypto;
mod storage;
mod counterparty;
mod eth_client;
mod channel_client;

use crypto::Crypto;

lazy_static! {
    pub static ref CRYPTO: Box<Crypto> = Box::new(Crypto::new());
}


fn main() {

}
