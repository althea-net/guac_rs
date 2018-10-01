use actix_web::http::Method;
use actix_web::*;

use guac_core::channel_client::types::{Channel, UpdateTx};

use guac_core::crypto::CryptoService;
use guac_core::{CRYPTO, STORAGE};

use guac_core::channel_client::ChannelManager;

use failure::Error;

use futures::Future;

use NetworkRequest;

use guac_core::counterparty::Counterparty;
use std::net::SocketAddr;

pub fn init_server(port: u16) {
    server::new(|| {
        App::new().resource("/update", |r| {
            r.method(Method::POST).with_async(update_endpoint)
        })
        // TODO: Uncomment this code once other errors are fixed
        // .resource("/propose", |r| {
        //     r.method(Method::POST).with_async(propose_channel_endpoint)
        // }).resource("/channel_created", |r| {
        //     r.method(Method::POST).with_async(channel_created_endpoint)
        // }).resource("/channel_joined", |r| {
        //     r.method(Method::POST).with_async(channel_joined_endpoint)
        // })
    }).bind(&format!("[::0]:{}", port))
    .unwrap()
    .start();
}

pub fn update_endpoint(
    update: Json<NetworkRequest<UpdateTx>>,
) -> impl Future<Item = Json<UpdateTx>, Error = Error> {
    trace!("got state update {:?}", update);
    STORAGE
        .get_channel(update.from_addr.clone())
        .and_then(move |mut channel_manager| {
            channel_manager.received_payment(&update.data)?;
            Ok(Json(channel_manager.create_payment()?))
        }).responder()
}

pub fn propose_channel_endpoint(
    (req, channel): (HttpRequest, Json<NetworkRequest<Channel>>),
) -> impl Future<Item = Json<bool>, Error = Error> {
    trace!(
        "got channel proposal {:?}, {:?}",
        channel,
        req.connection_info().remote()
    );
    let from: SocketAddr = req.connection_info().remote().unwrap().parse().unwrap();

    let to_url = format!("[{}]:4874", from.ip());

    let counterparty = Counterparty {
        address: channel.from_addr.clone(),
        url: to_url,
    };
    trace!("inserting state {:?}", counterparty);
    STORAGE
        .init_data(counterparty, ChannelManager::New)
        .then(|_| {
            STORAGE
                .get_channel(channel.from_addr.clone())
                .and_then(move |mut channel_manager| {
                    Ok(Json(channel_manager.check_proposal(&channel.data)?))
                }).responder()
        })
}

pub fn channel_created_endpoint(
    channel: Json<NetworkRequest<Channel>>,
) -> impl Future<Item = Json<bool>, Error = Error> {
    trace!("got channel created update {:?}", channel);
    STORAGE
        .get_channel(channel.from_addr.clone())
        .and_then(move |mut channel_manager| {
            channel_manager.channel_created(&channel.data, CRYPTO.own_eth_addr())?;
            Ok(Json(true))
        }).responder()
}

pub fn channel_joined_endpoint(
    channel: Json<NetworkRequest<Channel>>,
) -> impl Future<Item = Json<bool>, Error = Error> {
    trace!("got channel joined update {:?}", channel);
    STORAGE
        .get_channel(channel.from_addr.clone())
        .and_then(move |mut channel_manager| {
            channel_manager.channel_joined(&channel.data)?;
            Ok(Json(true))
        }).responder()
}
