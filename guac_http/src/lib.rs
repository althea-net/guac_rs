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

use actix::System;
use blockchain_client::BlockchainClient;
use clarity::utils::hex_str_to_bytes;
use clarity::{Address, PrivateKey};
use counterparty_client::CounterpartyClient;
use failure::Error;
use futures::{future, Future};
use guac_core::types::Counterparty;
use guac_core::UserApi;
use guac_core::{Crypto, Guac, Storage};
use std::sync::Arc;

#[macro_export]
macro_rules! try_future_box {
    ($expression:expr) => {
        match $expression {
            Err(err) => {
                return Box::new(future::err(err.into())) as Box<Future<Item = _, Error = Error>>;
            }
            Ok(value) => value,
        }
    };
}

pub fn init_guac(
    port: u16,
    contract_address: Address,
    own_address: Address,
    secret: PrivateKey,
    full_node_url: String,
) -> Guac {
    let guac = Guac {
        blockchain_client: Arc::new(Box::new(BlockchainClient::new(
            contract_address,
            own_address,
            secret,
            &full_node_url,
        ))),
        counterparty_client: Arc::new(Box::new(CounterpartyClient {})),
        storage: Arc::new(Box::new(Storage::new())),
        crypto: Arc::new(Box::new(Crypto {
            contract_address,
            own_address,
            secret,
        })),
    };

    counterparty_server::init_server(port, guac.clone());

    guac
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_counterparty() {
        let system = actix::System::new("test");

        let contract_addr: Address = "0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2"
            .parse()
            .unwrap();

        let pk_1: PrivateKey = "fafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafa"
            .parse()
            .unwrap();
        let addr_1 = pk_1.to_public_key().unwrap();

        let pk_2: PrivateKey = "0101010101010101010101010101010101010101010101010101010101010101"
            .parse()
            .unwrap();
        let addr_2 = pk_2.to_public_key().unwrap();

        let guac_1 = init_guac(8888, contract_addr, addr_1, pk_1, "example.com".to_string());

        let storage_1 = guac_1.storage.clone();

        actix::spawn(
            guac_1
                .register_counterparty(addr_2, "example.com".to_string())
                .then(move |res| {
                    res.unwrap();

                    assert_eq!(
                        storage_1.get_counterparty(addr_2).wait().unwrap().clone(),
                        Counterparty::New {
                            i_am_0: true,
                            url: "example.com".to_string()
                        }
                    );

                    System::current().stop();
                    Box::new(future::ok(()))
                }),
        );

        system.run();
    }
}
