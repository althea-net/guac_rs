#[cfg(test)]
use actix::actors::mocker::Mocker;
use actix::prelude::*;
use clarity::Address;
use clarity::Signature;
use failure::Error;
use guac_core::eth_client::{join_channel, open_channel, update_channel, ChannelId};
use num256::Uint256;

struct ChannelActorImpl;

impl Default for ChannelActorImpl {
    fn default() -> Self {
        Self {}
    }
}

impl Actor for ChannelActorImpl {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
        println!("Actor is alive");
    }

    fn stopped(&mut self, _ctx: &mut Context<Self>) {
        println!("Actor is stopped");
    }
}

#[derive(Debug)]
struct OpenChannel(Address, Uint256, Uint256);

impl Message for OpenChannel {
    type Result = Result<ChannelId, Error>;
}

impl Handler<OpenChannel> for ChannelActorImpl {
    type Result = ResponseFuture<ChannelId, Error>;

    fn handle(&mut self, msg: OpenChannel, _ctx: &mut Context<Self>) -> Self::Result {
        open_channel(msg.0, msg.1, msg.2)
    }
}

#[derive(Debug)]
struct JoinChannel(ChannelId, Uint256);

impl Message for JoinChannel {
    type Result = Result<(), Error>;
}

impl Handler<JoinChannel> for ChannelActorImpl {
    type Result = ResponseFuture<(), Error>;

    fn handle(&mut self, msg: JoinChannel, _ctx: &mut Context<Self>) -> Self::Result {
        join_channel(msg.0, msg.1)
    }
}

#[derive(Debug)]
struct UpdateChannel(ChannelId, Uint256, Uint256, Uint256, Signature, Signature);

impl Message for UpdateChannel {
    type Result = Result<(), Error>;
}

impl Handler<UpdateChannel> for ChannelActorImpl {
    type Result = ResponseFuture<(), Error>;

    fn handle(&mut self, msg: UpdateChannel, _ctx: &mut Context<Self>) -> Self::Result {
        update_channel(msg.0, msg.1, msg.2, msg.3, msg.4, msg.5)
    }
}

#[cfg(not(test))]
type ChannelActor = ChannelActorImpl;
#[cfg(test)]
type ChannelActor = Mocker<ChannelActorImpl>;

#[test]
fn does_it_work() {
    use futures::Future;
    use std::any::Any;

    // XXX: This is not necessarily a test but a more like a playground where I'm trying to get this stuff to compile
    let sys = System::new("test");
    let addr = ChannelActor::mock(Box::new(|v, _ctx| -> Box<Any> {
        if let Some(msg) = v.downcast_ref::<OpenChannel>() {
            println!("intercepted msg {:?}", msg);
            let mut channel_id: ChannelId = [42u8; 32];
            Box::new(Some(Ok(channel_id) as Result<ChannelId, Error>))
        } else {
            println!("I dont know that message");
            Box::new(None as Option<Result<ChannelId, Error>>)
        }
    })).start();
    let result = addr.send(OpenChannel(
        "0x4242424242424242424242424242424242424242"
            .parse()
            .unwrap(),
        Uint256::from(42u64),
        Uint256::from(1000u64),
    ));
    // spawn future to reactor
    Arbiter::spawn(
        result
            .map(|res| {
                match res {
                    Ok(result) => println!("Got result: {:?}", result),
                    Err(err) => println!("Got error: {}", err),
                }
                System::current().stop();
            }).map_err(|e| {
                println!("Actor is probably died: {}", e);
            }),
    );

    sys.run();
}
