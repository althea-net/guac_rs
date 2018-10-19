extern crate actix;
extern crate actix_web;
extern crate althea_types;
extern crate bytes;
extern crate clarity;
#[macro_use]
extern crate failure;
extern crate futures;
extern crate guac_core;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate num256;
extern crate qutex;
extern crate serde;
extern crate serde_json;
extern crate tokio;

use actix::prelude::*;
use actix_web::*;
use althea_types::PaymentTx;
use failure::Error;
use futures::Future;

use guac_core::channel_client::ChannelManager;
pub use guac_core::counterparty::Counterparty;
use guac_core::STORAGE;

pub use guac_core::crypto::CryptoService;
pub use guac_core::CRYPTO;

mod network_endpoints;
mod network_requests;

pub use network_endpoints::init_server;

use clarity::Address;
use network_requests::tick;
use num256::Uint256;
use std::ops::{Add, Sub};

/// A data type which wraps all network requests that guac makes, to check who the request is from
/// easily without request specific pattern matching
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkRequest<T> {
    pub from_addr: Address,
    pub data: T,
}

impl<T> NetworkRequest<T> {
    pub fn wrap(data: T) -> NetworkRequest<T> {
        NetworkRequest {
            from_addr: CRYPTO.own_eth_addr(),
            data,
        }
    }
}

pub struct PaymentController {}

impl Default for PaymentController {
    fn default() -> PaymentController {
        PaymentController {}
    }
}

impl Actor for PaymentController {
    type Context = Context<Self>;
}
impl Supervised for PaymentController {}
impl SystemService for PaymentController {
    fn service_started(&mut self, _ctx: &mut Context<Self>) {
        info!("Payment Controller started");
    }
}

#[derive(Clone, Debug)]
pub struct MakePayment(pub PaymentTx);

impl Message for MakePayment {
    type Result = Result<(), Error>;
}

impl Handler<MakePayment> for PaymentController {
    type Result = ResponseFuture<(), Error>;

    fn handle(&mut self, msg: MakePayment, _ctx: &mut Context<Self>) -> Self::Result {
        trace!("sending payment {:?}", msg);
        *CRYPTO.get_balance_mut() = CRYPTO
            .get_balance_mut()
            .clone()
            .sub(Uint256(msg.0.amount.clone()));
        Box::new(STORAGE.get_channel(msg.0.to.eth_address.clone()).and_then(
            move |mut channel_manager| {
                channel_manager.pay_counterparty(Uint256(msg.0.amount.clone()))?;
                Ok(())
            },
        ))
    }
}

#[derive(Clone)]
pub struct Tick;

impl Message for Tick {
    type Result = Result<(), Error>;
}

impl Handler<Tick> for PaymentController {
    type Result = ResponseFuture<(), Error>;

    fn handle(&mut self, _msg: Tick, _ctx: &mut Context<Self>) -> Self::Result {
        // TODO: Send to bounty hunter
        Box::new(STORAGE.get_all_counterparties().and_then(|keys| {
            for i in keys {
                Arbiter::spawn(tick(i.clone()).then(move |res| {
                    match res {
                        Ok(_) => {
                            info!("tick to {:?} was successful", i);
                        }
                        Err(e) => {
                            error!("tick to {:?} failed with {:?}", i, e);
                        }
                    };
                    Ok(())
                }));
            }
            Ok(())
        }))
    }
}

#[derive(Clone)]
pub struct Register(pub Counterparty);

impl Message for Register {
    type Result = Result<(), Error>;
}

impl Handler<Register> for PaymentController {
    type Result = ResponseFuture<(), Error>;

    fn handle(&mut self, msg: Register, _ctx: &mut Context<Self>) -> Self::Result {
        Box::new(STORAGE.init_data(msg.0, ChannelManager::New))
    }
}

/// This message needs to be sent periodically for every single address the application is
/// interested in, and it returns the amount of money we can consider to have "received"
/// from a counterparty
pub struct Withdraw(pub Address);

impl Message for Withdraw {
    type Result = Result<Uint256, Error>;
}

impl Handler<Withdraw> for PaymentController {
    type Result = ResponseFuture<Uint256, Error>;
    fn handle(&mut self, msg: Withdraw, _: &mut Context<Self>) -> Self::Result {
        Box::new(STORAGE.get_channel(msg.0.clone()).and_then(move |mut i| {
            let withdraw = i.withdraw()?;
            trace!("withdrew {:?} from {:?}", withdraw, &msg.0);
            *CRYPTO.get_balance_mut() = CRYPTO.get_balance().add(withdraw.clone());

            Ok(withdraw)
        }))
    }
}

pub struct GetOwnBalance;

impl Message for GetOwnBalance {
    type Result = Result<Uint256, Error>;
}

impl Handler<GetOwnBalance> for PaymentController {
    type Result = Result<Uint256, Error>;
    fn handle(&mut self, _msg: GetOwnBalance, _: &mut Context<Self>) -> Self::Result {
        Ok(CRYPTO.get_balance().clone())
    }
}
