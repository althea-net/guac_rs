extern crate actix;
extern crate futures;
extern crate guac_actix;

use actix::dev::{ContextParts, Mailbox};
use actix::prelude::*;
use futures::future::{ok, Future};
use guac_actix::GetOwnBalance;
use guac_actix::PaymentController;
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
