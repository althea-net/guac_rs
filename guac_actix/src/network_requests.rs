#[cfg(test)]
use actix::actors::mocker::Mocker;
use actix::prelude::*;

use guac_core::channel_client::types::{Channel, UpdateTx};
use guac_core::crypto::CryptoService;
use guac_core::CRYPTO;
use guac_core::STORAGE;

use failure::Error;
use futures;
use futures::future::result;
use futures::Future;
use futures::IntoFuture;

use channel_actor::{ChannelActor, OpenChannel};
use guac_core::channel_client::{ChannelManager, ChannelManagerAction};
use guac_core::counterparty::Counterparty;
use guac_core::transport_protocol::TransportProtocol;
use guac_core::transports::http::client::HTTPTransportClient;
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
                                )).from_err()
                                .and_then(move |channel_id| {
                                    trace!(
                                        "After open channel was sent {:?} ({:?})",
                                        channel_id,
                                        cm
                                    );
                                    // only when all the requests were successful, we commit it to `channel_manager`
                                    // (which makes the state change permanent)
                                    // channel_manager.received_channel_id();
                                    // CM should be in PendingCreation state
                                    let mut cm = cm.unwrap().unwrap();
                                    cm.channel_open_event(&Uint256::from(channel_id.unwrap()))?;
                                    *channel_manager = cm;
                                    Ok(())
                                })
                        }),
                )
                    as Box<Future<Item = (), Error = Error>>,
                ChannelManagerAction::SendNewChannelTransaction(_) => {
                    *channel_manager = temp_channel_manager;
                    Box::new(futures::future::ok(())) as Box<Future<Item = (), Error = Error>>
                }
                ChannelManagerAction::SendChannelJoinTransaction(channel) => Box::new(
                    NetworkRequestActor::from_registry()
                        .send(SendChannelJoined(
                            channel,
                            counterparty.url,
                            temp_channel_manager,
                        )).then(move |cm| {
                            *channel_manager = cm.unwrap().unwrap();
                            Ok(())
                        }),
                ),
                ChannelManagerAction::SendChannelCreatedUpdate(channel) => Box::new(
                    NetworkRequestActor::from_registry()
                        .send(SendChannelCreatedRequest(
                            channel,
                            counterparty.url,
                            temp_channel_manager,
                        )).then(move |cm| {
                            trace!(
                                "Send channel created requested returned old_cm={:?} new={:?}",
                                channel_manager,
                                cm
                            );
                            *channel_manager = cm.unwrap().unwrap();
                            Ok(())
                        }),
                )
                    as Box<Future<Item = (), Error = Error>>,
                ChannelManagerAction::SendUpdatedState(update) => Box::new(
                    NetworkRequestActor::from_registry()
                        .send(SendChannelUpdate(
                            update,
                            counterparty.url,
                            temp_channel_manager,
                        )).then(move |cm| {
                            *channel_manager = cm.unwrap().unwrap();
                            Ok(())
                        }),
                )
                    as Box<Future<Item = (), Error = Error>>,
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
        Box::new(
            result(HTTPTransportClient::new(msg.1.clone()))
                .from_err()
                .and_then(move |transport| {
                    transport
                        .send_channel_created_request(&msg.0.clone())
                        .from_err()
                        .and_then(move |_| Ok(msg.2.clone()))
                        .into_future()
                }),
        )
    }
}

#[derive(Debug)]
pub struct SendProposalRequest(pub Channel, pub String, pub ChannelManager);

impl Message for SendProposalRequest {
    type Result = Result<ChannelManager, Error>;
}

impl Handler<SendProposalRequest> for NetworkRequestActorImpl {
    type Result = ResponseFuture<ChannelManager, Error>;

    fn handle(&mut self, msg: SendProposalRequest, _ctx: &mut Context<Self>) -> Self::Result {
        Box::new(
            result(HTTPTransportClient::new(msg.1.clone()))
                .from_err()
                .and_then(move |transport| {
                    transport
                        .send_proposal_request(&msg.0.clone())
                        .from_err()
                        .and_then(move |res| {
                            let mut manager = msg.2.clone();
                            manager
                                .proposal_result(res, 0u64.into())
                                .and_then(move |_| Ok(manager))
                        }).into_future()
                }),
        )
    }
}

#[derive(Debug)]
pub struct SendChannelJoined(pub Channel, pub String, pub ChannelManager);

impl Message for SendChannelJoined {
    type Result = Result<ChannelManager, Error>;
}

impl Handler<SendChannelJoined> for NetworkRequestActorImpl {
    type Result = ResponseFuture<ChannelManager, Error>;

    fn handle(&mut self, msg: SendChannelJoined, _ctx: &mut Context<Self>) -> Self::Result {
        Box::new(
            result(HTTPTransportClient::new(msg.1.clone()))
                .from_err()
                .and_then(move |transport| {
                    transport
                        .send_channel_joined(&msg.0)
                        .from_err()
                        .and_then(|_| Ok(msg.2))
                        .into_future()
                }),
        )
    }
}

#[derive(Debug)]
pub struct SendChannelUpdate(pub UpdateTx, pub String, pub ChannelManager);

impl Message for SendChannelUpdate {
    type Result = Result<ChannelManager, Error>;
}

impl Handler<SendChannelUpdate> for NetworkRequestActorImpl {
    type Result = ResponseFuture<ChannelManager, Error>;

    fn handle(&mut self, msg: SendChannelUpdate, _ctx: &mut Context<Self>) -> Self::Result {
        Box::new(
            result(HTTPTransportClient::new(msg.1.clone()))
                .from_err()
                .and_then(move |transport| {
                    transport
                        .send_channel_update(&msg.0.clone())
                        .from_err()
                        .and_then(move |res| {
                            let mut manager = msg.2.clone();
                            manager
                                .received_updated_state(&res)
                                .and_then(|_| Ok(manager))
                        }).into_future()
                }),
        )
    }
}
