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

use guac_core::channel_client::{ChannelManager, ChannelManagerAction};
use guac_core::counterparty::Counterparty;

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
                    send_proposal_request(channel, counterparty.url, temp_channel_manager)
                        // we move the mutex guarded channel manager through to the closure, which
                        // ensures nothing else tampers with it
                        .and_then(move |cm| {
                            // only when the request is successful, we `commit` it to `channel_manager`
                            // (which makes the state change permanent)
                            *channel_manager = cm;
                            Ok(())
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
                    send_channel_created_request(channel, counterparty.url, temp_channel_manager)
                        .and_then(move |cm| {
                            *channel_manager = cm;
                            Ok(())
                        }),
                )
                    as Box<Future<Item = (), Error = Error>>,
                ChannelManagerAction::SendUpdatedState(update) => {
                    Box::new(
                        send_channel_update(update, counterparty.url, temp_channel_manager)
                            .and_then(move |cm| {
                                *channel_manager = cm;
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

pub fn send_channel_created_request(
    channel: Channel,
    url: String,
    manager: ChannelManager,
) -> impl Future<Item = ChannelManager, Error = Error> {
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

pub fn send_proposal_request(
    channel: Channel,
    url: String,
    mut manager: ChannelManager,
) -> impl Future<Item = ChannelManager, Error = Error> {
    let socket: SocketAddr = url.parse().unwrap();
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

pub fn send_channel_update(
    update: UpdateTx,
    url: String,
    mut manager: ChannelManager,
) -> impl Future<Item = ChannelManager, Error = Error> {
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
