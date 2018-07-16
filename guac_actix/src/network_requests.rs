use actix_web::client;
use actix_web::AsyncResponder;
use actix_web::HttpMessage;
use actix_web::Json;

use qutex::Guard;

use guac_core::channel_client::types::{Channel, UpdateTx};
use guac_core::CRYPTO;
use guac_core::STORAGE;

use failure::Error;
use futures;
use futures::Future;

use NetworkRequest;

use althea_types::Bytes32;
use guac_core::channel_client::{ChannelManager, ChannelManagerAction};
use guac_core::counterparty::Counterparty;

pub fn tick(counterparty: Counterparty) -> impl Future<Item = (), Error = Error> {
    STORAGE
        .get_channel(counterparty.address)
        .and_then(move |mut channel_manager| {
            let action = channel_manager
                .tick(counterparty.address, CRYPTO.own_eth_addr())
                .unwrap();

            match action {
                ChannelManagerAction::SendNewChannelTransaction(channel) => Box::new(
                    send_proposal_request(channel, counterparty.url, channel_manager),
                )
                    as Box<Future<Item = (), Error = Error>>,
                ChannelManagerAction::SendChannelJoinTransaction(_) => {
                    Box::new(futures::future::ok(())) as Box<Future<Item = (), Error = Error>>
                }

                ChannelManagerAction::SendChannelCreatedUpdate(channel) => Box::new(
                    send_channel_created_request(channel, counterparty.url, channel_manager),
                )
                    as Box<Future<Item = (), Error = Error>>,
                ChannelManagerAction::SendUpdatedState(update) => Box::new(send_channel_update(
                    update,
                    counterparty.url,
                    channel_manager,
                ))
                    as Box<Future<Item = (), Error = Error>>,
                ChannelManagerAction::None => {
                    Box::new(futures::future::ok(())) as Box<Future<Item = (), Error = Error>>
                }
            }
        })
}

pub fn send_channel_created_request(
    channel: Channel,
    url: String,
    manager: Guard<ChannelManager>,
) -> impl Future<Item = (), Error = Error> {
    client::post(&format!("{}/channel_created", url))
        .json(channel)
        .unwrap()
        .send()
        .from_err()
        .and_then(move |response| {
            response.body().from_err().and_then(move |res| {
                trace!("got {:?} back from sending channel_created to {}", res, url);
                Ok(())
            })
        })
}

pub fn send_proposal_request(
    channel: Channel,
    url: String,
    mut manager: Guard<ChannelManager>,
) -> impl Future<Item = (), Error = Error> {
    client::post(&format!("{}/propose", url))
        .json(channel)
        .unwrap()
        .send()
        .from_err()
        .and_then(move |response| {
            response
                .json()
                .from_err()
                .and_then(move |res: bool| manager.proposal_result(res))
        })
        .from_err()
}

pub fn send_channel_update(
    update: UpdateTx,
    url: String,
    mut manager: Guard<ChannelManager>,
) -> impl Future<Item = (), Error = Error> {
    client::post(&format!("{}/update", url))
        .json(update)
        .unwrap()
        .send()
        .from_err()
        .and_then(move |response| {
            response
                .json()
                .from_err()
                .and_then(move |res_update: UpdateTx| manager.rec_updated_state(&res_update))
        })
        .from_err()
}
