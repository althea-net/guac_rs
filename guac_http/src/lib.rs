// extern crate actix;
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
mod config;
mod counterparty_client;
mod counterparty_server;

use crate::blockchain_client::BlockchainClient;
use crate::counterparty_client::CounterpartyClient;
use clarity::{Address, PrivateKey};
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
    use crate::config::CONFIG;
    use actix::System;
    use failure::Error;
    use futures::{future, Future};
    use num256::Uint256;
    use std::cell::RefCell;
    use std::rc::Rc;
    use web3::client::Web3;

    fn eth_to_wei(eth: u64) -> Uint256 {
        let eth: Uint256 = eth.into();
        let mult = 1000000000000000000u64.into();
        eth * mult
    }

    fn make_nodes() -> (Guac, Guac) {
        let contract_addr: Address = CONFIG.contract_address.parse().unwrap();

        let pk_1: PrivateKey = CONFIG.private_key_0.parse().unwrap();
        let addr_1 = pk_1.to_public_key().unwrap();

        let pk_2: PrivateKey = CONFIG.private_key_1.parse().unwrap();
        let addr_2 = pk_2.to_public_key().unwrap();

        let guac_1 = init_guac(
            8881,
            contract_addr,
            addr_1,
            pk_1,
            "http://127.0.0.1:8545".to_string(),
        );
        let guac_2 = init_guac(
            8882,
            contract_addr,
            addr_2,
            pk_2,
            "http://127.0.0.1:8545".to_string(),
        );

        (guac_1, guac_2)
    }

    #[test]
    fn test_quick_deposit() {
        let system = actix::System::new("test");

        let (guac_1, guac_2) = make_nodes();

        let _storage_1 = guac_1.storage.clone();
        let web3 = Web3::new(&"http://127.0.0.1:8545".to_string());
        let web4 = Web3::new(&"http://127.0.0.1:8545".to_string());

        let snapshot_id: Rc<RefCell<Uint256>> = Rc::new(RefCell::new(0u64.into()));
        let snapshot_id_2 = snapshot_id.clone();

        actix::spawn(
            web3.evm_snapshot()
                .and_then(move |s| {
                    *snapshot_id.borrow_mut() = s;
                    make_and_fill_channel(guac_1, guac_2)
                })
                .then(move |res| {
                    let snapshot_id_2 = snapshot_id_2.borrow().clone();
                    web4.evm_revert(snapshot_id_2).then(|_| {
                        res.unwrap();

                        System::current().stop();
                        Box::new(future::ok(()))
                    })
                }),
        );

        system.run();
    }

    #[test]
    fn test_fill_channel() {
        let system = actix::System::new("test");

        let (guac_1, guac_2) = make_nodes();

        let _storage_1 = guac_1.storage.clone();
        let web3 = Web3::new(&"http://127.0.0.1:8545".to_string());
        let web4 = Web3::new(&"http://127.0.0.1:8545".to_string());

        let snapshot_id: Rc<RefCell<Uint256>> = Rc::new(RefCell::new(0u64.into()));
        let snapshot_id_2 = snapshot_id.clone();

        actix::spawn(
            web3.evm_snapshot()
                .and_then(move |s| {
                    *snapshot_id.borrow_mut() = s;
                    make_and_fill_channel(guac_1, guac_2)
                })
                .then(move |res| {
                    let snapshot_id_2 = snapshot_id_2.borrow().clone();
                    web4.evm_revert(snapshot_id_2).then(|_| {
                        res.unwrap();

                        System::current().stop();
                        Box::new(future::ok(()))
                    })
                }),
        );

        system.run();
    }

    fn make_and_fill_channel(guac_1: Guac, guac_2: Guac) -> Box<Future<Item = (), Error = Error>> {
        Box::new(
            guac_1
                .fill_channel(
                    guac_2.crypto.own_address,
                    "[::1]:8882".to_string(),
                    eth_to_wei(50),
                )
                .and_then(move |_| {
                    guac_2.fill_channel(
                        guac_1.crypto.own_address,
                        "[::1]:8881".to_string(),
                        eth_to_wei(50),
                    )
                }),
        )
    }

    #[test]
    fn test_make_payment_simple() {
        let system = actix::System::new("test");

        let (guac_1, guac_2) = make_nodes();

        let _storage_1 = guac_1.storage.clone();
        let web3 = Web3::new(&"http://127.0.0.1:8545".to_string());
        let web4 = Web3::new(&"http://127.0.0.1:8545".to_string());

        let snapshot_id: Rc<RefCell<Uint256>> = Rc::new(RefCell::new(0u64.into()));
        let snapshot_id_2 = snapshot_id.clone();

        actix::spawn(
            web3.evm_snapshot()
                .and_then(move |s| {
                    *snapshot_id.borrow_mut() = s;
                    make_and_fill_channel(guac_1.clone(), guac_2.clone()).and_then(move |_| {
                        guac_1.make_payment(
                            guac_2.crypto.own_address,
                            "[::1]:8882".to_string(),
                            eth_to_wei(1),
                        )
                    })
                })
                .then(move |res| {
                    let snapshot_id_2 = snapshot_id_2.borrow().clone();
                    web4.evm_revert(snapshot_id_2).then(|_| {
                        res.unwrap();

                        System::current().stop();
                        Box::new(future::ok(()))
                    })
                }),
        );

        system.run();
    }

    #[test]
    fn test_make_payment_packet_loss() {
        let system = actix::System::new("test");

        let (guac_1, guac_2) = make_nodes();

        let _storage_1 = guac_1.storage.clone();
        let web3 = Web3::new(&"http://127.0.0.1:8545".to_string());
        let web4 = Web3::new(&"http://127.0.0.1:8545".to_string());

        let snapshot_id: Rc<RefCell<Uint256>> = Rc::new(RefCell::new(0u64.into()));
        let snapshot_id_2 = snapshot_id.clone();

        actix::spawn(
            web3.evm_snapshot()
                .and_then(move |s| {
                    *snapshot_id.borrow_mut() = s;
                    make_and_fill_channel(guac_1.clone(), guac_2.clone()).and_then(move |_| {
                        guac_2
                            .make_payment(
                                guac_1.crypto.own_address,
                                // intentionally wrong address
                                "[::1]:8883".to_string(),
                                1u64.into(),
                            )
                            .then(move |_| {
                                guac_1.make_payment(
                                    guac_2.crypto.own_address,
                                    "[::1]:8882".to_string(),
                                    1u64.into(),
                                )
                            })
                    })
                })
                .then(move |res| {
                    let snapshot_id_2 = snapshot_id_2.borrow().clone();
                    web4.evm_revert(snapshot_id_2).then(|_| {
                        res.unwrap();

                        System::current().stop();
                        Box::new(future::ok(()))
                    })
                }),
        );

        system.run();
    }

    #[test]
    fn test_refill_channel() {
        let system = actix::System::new("test");

        let (guac_1, guac_2) = make_nodes();

        let _storage_1 = guac_1.storage.clone();
        let web3 = Web3::new(&"http://127.0.0.1:8545".to_string());
        let web4 = Web3::new(&"http://127.0.0.1:8545".to_string());

        let snapshot_id: Rc<RefCell<Uint256>> = Rc::new(RefCell::new(0u64.into()));
        let snapshot_id_2 = snapshot_id.clone();

        actix::spawn(
            web3.evm_snapshot()
                .and_then(move |s| {
                    *snapshot_id.borrow_mut() = s;
                    make_and_fill_channel(guac_1.clone(), guac_2.clone()).and_then(move |_| {
                        guac_1
                            .make_payment(
                                guac_2.crypto.own_address,
                                "[::1]:8882".to_string(),
                                1u64.into(),
                            )
                            .and_then(move |_| {
                                guac_1.fill_channel(
                                    guac_2.crypto.own_address,
                                    "[::1]:8882".to_string(),
                                    1u64.into(),
                                )
                            })
                    })
                })
                .then(move |res| {
                    let snapshot_id_2 = snapshot_id_2.borrow().clone();
                    web4.evm_revert(snapshot_id_2).then(|_| {
                        res.unwrap();

                        System::current().stop();
                        Box::new(future::ok(()))
                    })
                }),
        );

        system.run();
    }

    #[test]
    fn test_withdraw_channel() {
        let system = actix::System::new("test");

        let (guac_1, guac_2) = make_nodes();

        let _storage_1 = guac_1.storage.clone();
        let web3 = Web3::new(&"http://127.0.0.1:8545".to_string());
        let web4 = Web3::new(&"http://127.0.0.1:8545".to_string());

        let snapshot_id: Rc<RefCell<Uint256>> = Rc::new(RefCell::new(0u64.into()));
        let snapshot_id_2 = snapshot_id.clone();

        actix::spawn(
            web3.evm_snapshot()
                .and_then(move |s| {
                    *snapshot_id.borrow_mut() = s;
                    make_and_fill_channel(guac_1.clone(), guac_2.clone()).and_then(move |_| {
                        guac_1
                            .make_payment(
                                guac_2.crypto.own_address,
                                "[::1]:8882".to_string(),
                                eth_to_wei(10),
                            )
                            .and_then(move |_| {
                                guac_1
                                    .withdraw(
                                        guac_2.crypto.own_address,
                                        "[::1]:8882".to_string(),
                                        eth_to_wei(40),
                                    )
                                    .and_then(move |_| {
                                        web3.eth_get_balance(guac_1.crypto.own_address).and_then(
                                            move |balance| {
                                                println!("guac_1 balance: {:?}", balance);
                                                // assert_eq!(balance, eth_to_wei(9));
                                                guac_2
                                                    .withdraw(
                                                        guac_1.crypto.own_address,
                                                        "[::1]:8881".to_string(),
                                                        eth_to_wei(60),
                                                    )
                                                    .and_then(move |_| {
                                                        web3.eth_get_balance(
                                                            guac_2.crypto.own_address,
                                                        )
                                                        .and_then(move |balance| {
                                                            println!(
                                                                "guac_2 balance: {:?}",
                                                                balance
                                                            );
                                                            // assert_eq!(balance, eth_to_wei(11));
                                                            Ok(())
                                                        })
                                                    })
                                            },
                                        )
                                    })
                            })
                    })
                })
                .then(move |res| {
                    let snapshot_id_2 = snapshot_id_2.borrow().clone();
                    web4.evm_revert(snapshot_id_2).then(|_| {
                        res.unwrap();

                        System::current().stop();
                        Box::new(future::ok(()))
                    })
                }),
        );

        system.run();
    }

}
