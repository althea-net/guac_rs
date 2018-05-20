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
extern crate ethereum_types;
extern crate num256;
extern crate serde_json;
extern crate tiny_keccak;
extern crate uuid;
#[macro_use]
extern crate lazy_static;
extern crate ethabi;
extern crate ethcore_transaction;
#[macro_use]
extern crate ethabi_derive;
extern crate ethkey;
extern crate hex;
extern crate multihash;
extern crate qutex;
extern crate rlp;

use actix_web::{http, server, App};

mod channel_client;
mod counterparty;
mod crypto;
mod eth_client;
mod network_endpoints;
mod storage;

use network_endpoints::update;

use crypto::CRYPTO;
use storage::STORAGE;

fn main() {
    server::new(|| App::new().route("/update", http::Method::POST, update))
        .bind("127.0.0.1:8080")
        .unwrap()
        .run();
}
