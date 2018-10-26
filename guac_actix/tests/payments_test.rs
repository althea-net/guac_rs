extern crate actix;
extern crate futures;
extern crate guac_actix;
extern crate guac_core;

use actix::dev::{ContextParts, Mailbox};
use actix::prelude::*;
use futures::future::{ok, Future};
use guac_actix::GetOwnBalance;
use guac_actix::PaymentController;
use guac_actix::Register;
pub use guac_core::counterparty::Counterparty;
use std::thread;

#[test]
fn get_own_balance() {
    let system = System::new("test");
    let addr = PaymentController::default().start();
    let res = addr.send(GetOwnBalance);
    Arbiter::spawn(res.then(|res| {
        System::current().stop();
        ok(())
    }));
    system.run();
}

#[test]
fn register() {
    let system = System::new("test");
    let addr = PaymentController::default().start();
    let res = addr.send(Register(Counterparty {
        address: "0x0101010101010101010101010101010101010101"
            .parse()
            .unwrap(),
        url: "http://127.0.0.1:1234/".to_string(),
    }));
    Arbiter::spawn(res.then(|res| {
        println!("res {:?}", res);
        System::current().stop();
        ok(())
    }));
    system.run();
}
