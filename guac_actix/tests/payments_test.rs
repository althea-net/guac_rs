extern crate actix;
extern crate althea_types;
extern crate clarity;
extern crate futures;
extern crate guac_actix;
extern crate guac_core;
extern crate num256;

use actix::dev::{ContextParts, Mailbox};
use actix::prelude::*;
use althea_types::{Identity, PaymentTx};
use clarity::Address;
use futures::future::{ok, Future};
use guac_actix::GetOwnBalance;
use guac_actix::MakePayment;
use guac_actix::PaymentController;
use guac_actix::Register;
pub use guac_core::counterparty::Counterparty;
use num256::Uint256;
use std::net::{IpAddr, Ipv6Addr};
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

fn new_addr(x: u64) -> Address {
    format!("0x{}", format!("{:02}", x).repeat(20))
        .parse()
        .unwrap()
}

fn new_identity(x: u64) -> Identity {
    let y = x as u16;
    Identity {
        mesh_ip: IpAddr::V6(Ipv6Addr::new(y, y, y, y, y, y, y, y)),
        wg_public_key: String::from("AAAAAAAAAAAAAAAAAAAA"),
        eth_address: new_addr(x),
    }
}

#[test]
fn make_payment() {
    let system = System::new("test");
    let addr = PaymentController::default().start();
    let res = addr.send(MakePayment(PaymentTx {
        amount: 123u64.into(),
        from: new_identity(1),
        to: new_identity(2),
    }));
    Arbiter::spawn(res.then(|res| {
        println!("res {:?}", res);
        System::current().stop();
        ok(())
    }));
    system.run();
}
