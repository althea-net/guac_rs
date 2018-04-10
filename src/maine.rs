extern crate actix;
extern crate futures;

use actix::prelude::*;
use futures::Future;
use std::io;

/// Define message
struct Ping;

impl Message for Ping {
    type Result = Result<bool, io::Error>;
}

struct Pong;

impl Message for Pong {
    type Result = Result<bool, io::Error>;
}

// Define actor
struct MyActor
where
    A: Actor,
{
    other: actix::Addr<actix::Unsync, A>,
}

// Provide Actor implementation for our actor
impl Actor for MyActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        println!("Actor is alive");
    }

    fn stopped(&mut self, ctx: &mut Context<Self>) {
        println!("Actor is stopped");
    }
}

struct OtherActor;

impl Actor for OtherActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        println!("OtherActor is alive");
    }

    fn stopped(&mut self, ctx: &mut Context<Self>) {
        println!("OtherActor is stopped");
    }
}

struct FakeOtherActor;

impl Actor for FakeOtherActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        println!("FakeOtherActor is alive");
    }

    fn stopped(&mut self, ctx: &mut Context<Self>) {
        println!("FakeOtherActor is stopped");
    }
}

/// Define handler for `Ping` message
impl Handler<Ping> for MyActor {
    type Result = Result<bool, io::Error>;

    fn handle(&mut self, msg: Ping, ctx: &mut Context<Self>) -> Self::Result {
        println!("Ping received");

        Ok(true)
    }
}

impl Handler<Pong> for OtherActor {
    type Result = Result<bool, io::Error>;

    fn handle(&mut self, msg: Pong, ctx: &mut Context<Self>) -> Self::Result {
        println!("Pong received by other actor");

        Ok(true)
    }
}

impl Handler<Pong> for FakeOtherActor {
    type Result = Result<bool, io::Error>;

    fn handle(&mut self, msg: Pong, ctx: &mut Context<Self>) -> Self::Result {
        println!("Pong received by fake other actor");

        Ok(true)
    }
}

fn main() {
    let sys = System::new("example");

    // Start MyActor in current thread
    let otherAddr: Addr<Unsync, _> = OtherActor.start();

    let myAddr: Addr<Unsync, _> =
        MyActor::create(|ctx: &mut Context<MyActor>| MyActor { other: otherAddr });

    // let myAddr: Addr<Unsync, _> = MyActor.start();

    // Send Ping message.
    // send() message returns Future object, that resolves to message result
    let result = myAddr.send(Ping);

    // spawn future to reactor
    Arbiter::handle().spawn(
        result
            .map(|res| match res {
                Ok(result) => println!("Got result: {}", result),
                Err(err) => println!("Got error: {}", err),
            })
            .map_err(|e| {
                println!("Actor is probably died: {}", e);
            }),
    );

    sys.run();
}
