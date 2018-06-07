extern crate actix;
extern crate actix_web;
extern crate failure;
extern crate guac_core;
extern crate futures;
extern crate althea_types;
#[macro_use]
extern crate log;

use failure::Error;
use guac_core::channel_client::types::UpdateTx;
use althea_types::PaymentTx;
use actix::prelude::*;

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

#[derive(Message, Clone)]
pub struct MakePayment(pub PaymentTx);

impl Handler<MakePayment> for PaymentController {
    type Result = ();

    fn handle(&mut self, msg: MakePayment, ctx: &mut Context<Self>) -> Self::Result {

    }
}

#[derive(Message)]
pub struct PaymentControllerUpdate;

impl Handler<PaymentControllerUpdate> for PaymentController {
    type Result = ();

    fn handle(&mut self, msg: PaymentControllerUpdate, ctx: &mut Context<Self>) -> Self::Result {

    }
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