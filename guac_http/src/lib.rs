extern crate actix;
extern crate actix_web;
extern crate althea_types;
extern crate bytes;
extern crate clarity;
#[macro_use]
extern crate failure;
extern crate futures;
extern crate guac_core;

extern crate num256;
extern crate qutex;
extern crate serde;
extern crate serde_json;
extern crate tokio;
extern crate web3;

mod blockchain_client;
mod counterparty_client;
mod counterparty_server;

use blockchain_client::BlockchainClient;
use clarity::{Address, PrivateKey};
use counterparty_client::CounterpartyClient;
use guac_core::{Crypto, Guac, Storage};
use std::sync::Arc;

pub fn init_guac(
    contract_address: Address,
    own_address: Address,
    secret: PrivateKey,
    full_node_url: &String,
) -> Guac {
    let guac = Guac {
        blockchain_client: Arc::new(Box::new(BlockchainClient::new(
            contract_address,
            own_address,
            secret,
            full_node_url,
        ))),
        counterparty_client: Arc::new(Box::new(CounterpartyClient {})),
        storage: Arc::new(Box::new(Storage::new())),
        crypto: Arc::new(Box::new(Crypto {
            contract_address,
            own_address,
            secret,
        })),
    };

    counterparty_server::init_server(8888u16, guac.clone());

    guac
}
