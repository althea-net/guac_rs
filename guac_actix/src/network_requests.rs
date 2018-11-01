#[cfg(test)]
use actix::actors::mocker::Mocker;
use actix::prelude::*;
use actix_web::client;
use actix_web::client::Connection;
use actix_web::HttpMessage;

use guac_core::channel_client::types::{Channel, UpdateTx};
use guac_core::crypto::CryptoService;
use guac_core::CRYPTO;
use guac_core::STORAGE;

use failure::Error;
use futures;
use futures::Future;

use NetworkRequest;

use std::net::SocketAddr;

use tokio::net::TcpStream as TokioTcpStream;

use channel_actor::{ChannelActor, OpenChannel};
use futures::future::ok;
use guac_core::channel_client::{ChannelManager, ChannelManagerAction};
use guac_core::counterparty::Counterparty;
use num256::Uint256;

/// This function needs to be called periodically for every counterparty to to do things which
/// happen on a cycle.
pub fn tick(counterparty: Counterparty) -> impl Future<Item = (), Error = Error> {
    STORAGE
        .get_channel(counterparty.address.clone())
        .and_then(move |mut channel_manager| {
            // The channel_manager here is a mutex guard of a ChannelManager, which ensures that
            // the data within is not tampered with in the rest of the program while these requests
            // are outstanding

            // we copy the channel_manager and do all the mutations on the clone because we only
            // "commit" the changes when the tick is successful (could include network requests,
            // etc.)
            let mut temp_channel_manager = channel_manager.clone();
            trace!(
                "Tick: got channel for counterparty {:?} = {:?}",
                counterparty,
                temp_channel_manager
            );

            trace!(
                "counterparty {:?} is in state {:?}",
                counterparty.clone(),
                temp_channel_manager
            );
            let action = temp_channel_manager
                .tick(CRYPTO.own_eth_addr(), counterparty.address.clone())
                .unwrap();
            trace!(
                "counterparty {:?} is in state {:?} after update, returning action {:?}",
                counterparty.clone(),
                temp_channel_manager,
                action
            );

            // All the methods which are called per action take a channel manager, which may or may
            // not be mutated during the lifecycle of the request
            match action {
                ChannelManagerAction::SendChannelProposal(channel) => Box::new(
                    // This will do an HTTP request on other party HTTP server
                    // i.e. POST http://bob:1234/propose
                    NetworkRequestActor::from_registry()
                        .send(SendProposalRequest(
                            channel.clone(),
                            counterparty.url,
                            temp_channel_manager,
                        ))
                        // we move the mutex guarded channel manager through to the closure, which
                        // ensures nothing else tampers with it
                        .then(move |cm| {
                            trace!(
                                "After send proposal request channel={:?} cm={:?}",
                                channel.clone(),
                                cm
                            );
                            // Create a channel by contacting a channel actor after this request
                            // was successful
                            ChannelActor::from_registry()
                                .send(OpenChannel(
                                    channel.address_b.clone(),
                                    Uint256::from(42u64),
                                    Uint256::from(100_000_000_000_000u64),
                                ))
                                .then(move |channel_id| {
                                    trace!("After open channel was sent {:?} ({:?})", channel_id, cm);
                                    // only when all the requests were successful, we commit it to `channel_manager`
                                    // (which makes the state change permanent)
                                    *channel_manager = cm.unwrap().unwrap();
                                    ok(())
                                })
                        }),
                )
                    as Box<Future<Item = (), Error = Error>>,
                ChannelManagerAction::SendNewChannelTransaction(_) => {
                    *channel_manager = temp_channel_manager;
                    Box::new(futures::future::ok(())) as Box<Future<Item = (), Error = Error>>
                }
                ChannelManagerAction::SendChannelJoinTransaction(channel) => {
                    Box::new(
                        send_channel_joined(channel, counterparty.url, temp_channel_manager)
                            .and_then(move |cm| {
                                *channel_manager = cm;
                                Ok(())
                            }),
                    ) as Box<Future<Item = (), Error = Error>>
                }

                ChannelManagerAction::SendChannelCreatedUpdate(channel) => Box::new(
                     NetworkRequestActor::from_registry()
                        .send(SendChannelCreatedRequest(channel, counterparty.url, temp_channel_manager))
                        .then(move |cm| {
                            trace!("Send channel created requested returned old_cm={:?} new={:?}", channel_manager, cm);
                            *channel_manager = cm.unwrap().unwrap();
                            Ok(())
                        }),
                )
                    as Box<Future<Item = (), Error = Error>>,
                ChannelManagerAction::SendUpdatedState(update) => {
                    Box::new(
                        NetworkRequestActor::from_registry()
                        .send(SendChannelUpdate(update, counterparty.url, temp_channel_manager))
                            .then(move |cm| {
                                *channel_manager = cm.unwrap().unwrap();
                                Ok(())
                            }),
                    ) as Box<Future<Item = (), Error = Error>>
                }
                ChannelManagerAction::None => {
                    *channel_manager = temp_channel_manager;
                    Box::new(futures::future::ok(())) as Box<Future<Item = (), Error = Error>>
                }
            }
        })
}

#[cfg(not(test))]
pub type NetworkRequestActor = NetworkRequestActorImpl;
#[cfg(test)]
pub type NetworkRequestActor = Mocker<NetworkRequestActorImpl>;

/// An actor that is responsible for communication with other parties through HTTP.
pub struct NetworkRequestActorImpl;

impl Default for NetworkRequestActorImpl {
    fn default() -> Self {
        Self {}
    }
}

impl Supervised for NetworkRequestActorImpl {}

impl SystemService for NetworkRequestActorImpl {
    fn service_started(&mut self, _ctx: &mut Context<Self>) {
        info!("Network request actor system service started");
    }
}
impl Actor for NetworkRequestActorImpl {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
        trace!("Network request actor is alive");
    }

    fn stopped(&mut self, _ctx: &mut Context<Self>) {
        trace!("Network request actor is stopped");
    }
}

#[derive(Debug)]
pub struct SendChannelCreatedRequest(pub Channel, pub String, pub ChannelManager);

impl Message for SendChannelCreatedRequest {
    type Result = Result<ChannelManager, Error>;
}

impl Handler<SendChannelCreatedRequest> for NetworkRequestActorImpl {
    type Result = ResponseFuture<ChannelManager, Error>;

    fn handle(&mut self, msg: SendChannelCreatedRequest, _ctx: &mut Context<Self>) -> Self::Result {
        Box::new(send_channel_created_request(msg.0, msg.1, msg.2))
    }
}

fn send_channel_created_request(
    channel: Channel,
    url: String,
    manager: ChannelManager,
) -> impl Future<Item = ChannelManager, Error = Error> {
    trace!(
        "network_requests.rs - Send created request channel={:?} url={} manager={:?}",
        channel,
        url,
        manager
    );
    let socket: SocketAddr = url.parse().unwrap();
    let endpoint = format!("http://[{}]:{}/channel_created", socket.ip(), socket.port());

    let stream = TokioTcpStream::connect(&socket);

    stream.from_err().and_then(move |stream| {
        client::post(&endpoint)
            .with_connection(Connection::from_stream(stream))
            .json(NetworkRequest::wrap(channel))
            .unwrap()
            .send()
            .from_err()
            .and_then(move |response| {
                response.body().from_err().and_then(move |res| {
                    trace!("got {:?} back from sending channel_created to {}", res, url);
                    Ok(manager)
                })
            })
    })
}

#[derive(Debug)]
pub struct SendProposalRequest(pub Channel, pub String, pub ChannelManager);

impl Message for SendProposalRequest {
    type Result = Result<ChannelManager, Error>;
}

impl Handler<SendProposalRequest> for NetworkRequestActorImpl {
    type Result = ResponseFuture<ChannelManager, Error>;

    fn handle(&mut self, msg: SendProposalRequest, _ctx: &mut Context<Self>) -> Self::Result {
        Box::new(send_proposal_request(msg.0, msg.1, msg.2))
    }
}

pub fn send_proposal_request(
    channel: Channel,
    url: String,
    mut manager: ChannelManager,
) -> impl Future<Item = ChannelManager, Error = Error> {
    trace!(
        "network_requests.rs - Send channel proposal request channel={:?} url={} manager={:?}",
        channel,
        url,
        manager
    );
    let socket: SocketAddr = url.parse().expect("Unable to parse URL");
    let endpoint = format!("http://[{}]:{}/propose", socket.ip(), socket.port());

    let stream = TokioTcpStream::connect(&socket);

    stream.from_err().and_then(move |stream| {
        client::post(&endpoint)
            .with_connection(Connection::from_stream(stream))
            .json(NetworkRequest::wrap(channel))
            .unwrap()
            .send()
            .from_err()
            .and_then(move |response| {
                response.json().from_err().and_then(move |res: bool| {
                    manager.proposal_result(res)?;
                    Ok(manager)
                })
            })
    })
}

#[derive(Debug)]
pub struct SendChannelUpdate(pub UpdateTx, pub String, pub ChannelManager);

impl Message for SendChannelUpdate {
    type Result = Result<ChannelManager, Error>;
}

impl Handler<SendChannelUpdate> for NetworkRequestActorImpl {
    type Result = ResponseFuture<ChannelManager, Error>;

    fn handle(&mut self, msg: SendChannelUpdate, _ctx: &mut Context<Self>) -> Self::Result {
        Box::new(send_channel_update(msg.0, msg.1, msg.2))
    }
}

fn send_channel_update(
    update: UpdateTx,
    url: String,
    mut manager: ChannelManager,
) -> impl Future<Item = ChannelManager, Error = Error> {
    trace!(
        "network_requests.rs - Send channel update request update={:?} url={} manager={:?}",
        update,
        url,
        manager
    );
    let socket: SocketAddr = url.parse().unwrap();
    let endpoint = format!("http://[{}]:{}/update", socket.ip(), socket.port());

    let stream = TokioTcpStream::connect(&socket);

    stream.from_err().and_then(move |stream| {
        client::post(&endpoint)
            .with_connection(Connection::from_stream(stream))
            .json(NetworkRequest::wrap(update))
            .unwrap()
            .send()
            .from_err()
            .and_then(move |response| {
                response
                    .json()
                    .from_err()
                    .and_then(move |res_update: UpdateTx| {
                        manager.received_updated_state(&res_update)?;
                        Ok(manager)
                    })
            })
    })
}

pub fn send_channel_joined(
    new_channel: Channel,
    url: String,
    manager: ChannelManager,
) -> impl Future<Item = ChannelManager, Error = Error> {
    trace!(
        "network_requests.rs - Send channel joined request channel={:?} url={} manager={:?}",
        new_channel,
        url,
        manager
    );
    let socket: SocketAddr = url.parse().unwrap();
    let endpoint = format!("http://[{}]:{}/channel_joined", socket.ip(), socket.port());

    let stream = TokioTcpStream::connect(&socket);

    stream.from_err().and_then(move |stream| {
        client::post(&endpoint)
            .with_connection(Connection::from_stream(stream))
            .json(NetworkRequest::wrap(new_channel))
            .unwrap()
            .send()
            .from_err()
            .and_then(move |response| {
                response.body().from_err().and_then(move |res| {
                    trace!("got {:?} back from sending pip to {}", res, url);
                    Ok(manager)
                })
            })
    })
}
