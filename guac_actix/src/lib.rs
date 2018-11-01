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

use guac_core::channel_client::types::UpdateTx;
use guac_core::channel_client::ChannelManager;
use guac_core::counterparty::Counterparty;
use guac_core::STORAGE;

pub use guac_core::crypto::CryptoService;
pub use guac_core::CRYPTO;

mod channel_actor;
mod network_endpoints;
mod network_requests;

pub use network_endpoints::init_server;

use actix::dev::{ContextParts, Mailbox};
use actix::prelude::*;
use althea_types::Identity;
use channel_actor::{ChannelActor, OpenChannel};
use clarity::Address;
use clarity::Signature;
use futures::future::ok;
use guac_core::eth_client::ChannelId;
use network_requests::tick;
use network_requests::{
    NetworkRequestActor, SendChannelCreatedRequest, SendChannelUpdate, SendProposalRequest,
};
use num256::Uint256;
use std::any::Any;
use std::net::{IpAddr, Ipv6Addr};
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
        *CRYPTO.get_balance_mut() -= Uint256(msg.0.amount.clone());
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
        trace!("Received a tick message");
        Box::new(STORAGE.get_all_counterparties().and_then(|keys| {
            trace!("Counterparties: {:?}", keys);
            for i in keys {
                trace!("Spawn tick for {:?}", i);
                Arbiter::spawn(tick(i.clone()).then(move |res| {
                    trace!("Tick result {:?}", res);
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

#[derive(Clone, Debug)]
pub struct Register(pub Counterparty);

impl Message for Register {
    type Result = Result<(), Error>;
}

impl Handler<Register> for PaymentController {
    type Result = ResponseFuture<(), Error>;

    fn handle(&mut self, msg: Register, _ctx: &mut Context<Self>) -> Self::Result {
        trace!("Register new counterparty {:?}", msg);
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

// extern crate log;

use log::{Level, LevelFilter, Metadata, Record};

struct ConsoleLogger;

impl log::Log for ConsoleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

#[test]
fn make_payment() {
    use std::sync::{Arc, Mutex};

    // TODO: There must be a better way to do this
    log::set_logger(&ConsoleLogger).unwrap();
    log::set_max_level(LevelFilter::Trace);

    let system = System::new("test");
    let addr = PaymentController::default().start();

    // Keeps track and order of ChannelActor messages
    let mut channel_call_counter = Arc::new(Mutex::new(0));
    // Keeps track and order of NetworkRequestActor messages
    let mut network_requests_counter = Arc::new(Mutex::new(0));

    let channel_counter = channel_call_counter.clone();
    let network_counter = network_requests_counter.clone();

    let channel_addr = ChannelActor::mock(Box::new(move |v, _ctx| -> Box<Any> {
        let mut channel_counter = channel_counter.lock().unwrap();
        let mut network_counter = network_counter.lock().unwrap();

        *channel_counter += 1;
        println!("Channel actor received {} msg", *channel_counter);
        if let Some(msg) = v.downcast_ref::<OpenChannel>() {
            // Verify that this happens *before* network request.
            // This is first call...
            assert_eq!(*channel_counter, 1);
            // and no network requests are received yet
            assert_eq!(*network_counter, 1);

            println!("intercepted {:?}", msg);
            let mut channel_id: ChannelId = [42u8; 32];
            Box::new(Some(Ok(channel_id) as Result<ChannelId, Error>))
        } else if let Some(msg) = v.downcast_ref::<SendChannelCreatedRequest>() {
            // This is the 2nd call
            assert_eq!(*channel_counter, 2);
            println!("intercepted {:?}", msg);
            let cm = msg.2.clone();
            Box::new(Some(Ok(cm) as Result<ChannelManager, Error>))
        } else {
            println!("I dont know that message");
            Box::new(None as Option<Result<ChannelId, Error>>)
        }
    })).start();
    System::current().registry().set(channel_addr);

    let channel_counter = channel_call_counter.clone();
    let network_counter = network_requests_counter.clone();
    let network_request_addr = NetworkRequestActor::mock(Box::new(move |v, _ctx| -> Box<Any> {
        let mut channel_counter = channel_counter.lock().unwrap();
        let mut network_counter = network_counter.lock().unwrap();

        println!("Network requests received {} msg", *network_counter);
        *network_counter += 1;

        if let Some(msg) = v.downcast_ref::<SendProposalRequest>() {
            println!("intercepted network request msg {:?}", msg);
            // Channel call didn't happen yet as we're proposing now
            assert_eq!(*channel_counter, 0);
            // This is the first network request
            assert_eq!(*network_counter, 1);

            let mut cm = msg.2.clone();
            cm.proposal_result(true)
                .expect("Proposal result was expected to succeed");
            Box::new(Some(Ok(cm) as Result<ChannelManager, Error>))
        } else if let Some(msg) = v.downcast_ref::<SendChannelCreatedRequest>() {
            println!("intercepted send channel created request msg {:?}", msg);
            assert_eq!(*channel_counter, 1);
            assert_eq!(*network_counter, 2);

            let mut cm = msg.2.clone();
            Box::new(Some(Ok(cm) as Result<ChannelManager, Error>))
        } else if let Some(msg) = v.downcast_ref::<SendChannelUpdate>() {
            println!("intercept send updated state request msg {:?}", msg);
            assert_eq!(*channel_counter, 1);
            assert_eq!(*network_counter, 3);
            let mut cm = msg.2.clone();

            cm.received_updated_state(&UpdateTx {
                channel_id: "0x4242424242424242424242424242424242424242"
                    .parse()
                    .unwrap(),
                nonce: Uint256::from(123u64),

                balance_a: Uint256::from(1u64),
                balance_b: Uint256::from(2u64),

                signature_a: Some(Signature::new(1u64.into(), 2u64.into(), 3u64.into())),
                signature_b: Some(Signature::new(4u64.into(), 5u64.into(), 6u64.into())),
            }).expect("Recevied update state");

            Box::new(Some(Ok(cm) as Result<ChannelManager, Error>))
        } else {
            println!("intercepted unknown network manager msg");
            Box::new(None as Option<Result<ChannelManager, Error>>)
        }
    })).start();
    System::current().registry().set(network_request_addr);
    let res = addr.send(Register(Counterparty {
        address: "0x4242424242424242424242424242424242424242"
            .parse()
            .unwrap(),
        url: "http://127.0.0.1:12345".to_string(),
    }));
    Arbiter::spawn(
        res.then(|res| {
            println!("res {:?}", res);
            PaymentController::from_registry().send(Tick)
        }).then(|res| {
            println!("tick1 result {:?}", res);
            println!("------------------------------------- tick 2");
            PaymentController::from_registry().send(Tick)
        }).then(|res| {
            println!("tick2 result {:?}", res);
            println!("------------------------------------- tick 3");
            PaymentController::from_registry().send(Tick)
        }).then(|res| {
            println!("tick3 result {:?}", res);
            println!("------------------------------------- tick 4");
            PaymentController::from_registry().send(Tick)
        }).then(|res| {
            println!("tick4 result {:?}", res);
            System::current().stop();
            ok(())
        }),
    );

    system.run();

    assert_eq!(STORAGE.get_all_counterparties().wait().unwrap().len(), 1);
    println!(
        "All counterparties {:?}",
        STORAGE.get_all_counterparties().wait().unwrap()
    );
}
