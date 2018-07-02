extern crate actix;
extern crate actix_web;
extern crate althea_types;
extern crate failure;
extern crate futures;
extern crate guac_core;
#[macro_use]
extern crate log;

use actix::prelude::*;
use actix_web::*;
use althea_types::EthAddress;
use althea_types::PaymentTx;
use failure::Error;
use futures::Future;

use guac_core::channel_client::types::UpdateTx;
use guac_core::counterparty::Counterparty;
use guac_core::STORAGE;

pub struct PaymentController {}

impl Default for PaymentController {
    fn default() -> PaymentController {
        unimplemented!()
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

#[derive(Message)]
pub struct PaymentReceived(pub UpdateTx);

impl Handler<PaymentReceived> for PaymentController {
    type Result = ();

    fn handle(&mut self, msg: PaymentReceived, _: &mut Context<Self>) -> Self::Result {
        ()
    }
}

#[derive(Clone)]
pub struct MakePayment(pub PaymentTx);

impl Message for MakePayment {
    type Result = Result<(), Error>;
}

impl Handler<MakePayment> for PaymentController {
    type Result = ResponseFuture<(), Error>;

    fn handle(&mut self, msg: MakePayment, ctx: &mut Context<Self>) -> Self::Result {
        Box::new(
            STORAGE
                .get_channel(msg.0.to.eth_address)
                .and_then(move |mut channel_manager| {
                    channel_manager.pay_counterparty(msg.0.amount)?;
                    Ok(())
                }),
        )
    }
}

#[derive(Clone)]
pub struct Tick;

impl Message for Tick {
    type Result = Result<(), Error>;
}

impl Handler<Tick> for PaymentController {
    type Result = ResponseFuture<(), Error>;

    fn handle(&mut self, _msg: Tick, ctx: &mut Context<Self>) -> Self::Result {
        // TODO: Send to bounty hunter
        Box::new(STORAGE.get_all_counterparties().and_then(|keys| {
            for i in keys {
                Arbiter::handle().spawn(send_payment(i.clone()).then(move |res| {
                    match res {
                        Ok(_) => {
                            info!("payment request to {:?} was successful", i);
                        }
                        Err(e) => {
                            error!("payment request to {:?} failed with {:?}", i, e);
                        }
                    };
                    Ok(())
                }));
            }
            Ok(())
        }))
    }
}

fn send_payment(counterparty: Counterparty) -> impl Future<Item = (), Error = Error> {
    STORAGE
        .get_channel(counterparty.address)
        .and_then(move |mut channel_manager| {
            let sent_update = channel_manager.create_payment().unwrap();
            client::post(&format!("{}/update", counterparty.url))
                .json(sent_update)
                .unwrap()
                .send()
                .from_err()
                .and_then(move |response| {
                    response
                        .json()
                        .from_err()
                        .and_then(move |res_update: UpdateTx| {
                            channel_manager.rec_updated_state(res_update)
                        })
                })
                .from_err()
        })
}

/*
#[derive(Clone)]
pub struct Register(EthAddress);

impl Message for Register {
    type Result = Result<(), Error>;
}

impl Handler<Register> for PaymentController {
    type Result = ResponseFuture<(), Error>;

    fn handle(&mut self, _msg: Register, ctx: &mut Context<Self>) -> Self::Result {
        Box::new(STORAGE.init_data())
    }
} */

#[derive(Message)]
pub struct PaymentControllerUpdate;

impl Handler<PaymentControllerUpdate> for PaymentController {
    type Result = ();

    fn handle(&mut self, msg: PaymentControllerUpdate, ctx: &mut Context<Self>) -> Self::Result {}
}

pub struct GetOwnBalance;

impl Message for GetOwnBalance {
    type Result = Result<i64, Error>;
}

impl Handler<GetOwnBalance> for PaymentController {
    type Result = Result<i64, Error>;
    fn handle(&mut self, _msg: GetOwnBalance, _: &mut Context<Self>) -> Self::Result {
        Ok(0)
    }
}
