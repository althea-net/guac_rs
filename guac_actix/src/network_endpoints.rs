use actix_web::server::HttpServer;
use actix_web::*;

use actix::*;

use guac_core::channel_client::types::{Channel, UpdateTx};

use guac_core::crypto::CryptoService;
use guac_core::{CRYPTO, STORAGE};

use failure::Error;

use futures::{self, Future};

use NetworkRequest;

use althea_types::Bytes32;
use guac_core::counterparty::Counterparty;

pub fn init_server() {
    server::new(|| {
        App::new()
            .resource("/update", |r| r.with_async(update_endpoint))
            .resource("/propose", |r| r.with_async(propose_channel_endpoint))
            .resource("/channel_created", |r| {
                r.with_async(channel_created_endpoint)
            })
    }).bind("127.0.0.1:8080")
        .unwrap()
        .start();
}

pub fn update_endpoint(
    update: Json<NetworkRequest<UpdateTx>>,
) -> impl Future<Item = Json<UpdateTx>, Error = Error> {
    Box::new(
        STORAGE
            .get_channel(update.from_addr.clone())
            .and_then(move |mut channel_manager| {
                channel_manager.rec_payment(&update.data)?;
                Ok(Json(channel_manager.create_payment()?))
            })
            .responder(),
    )
}

pub fn propose_channel_endpoint(
    channel: Json<NetworkRequest<Channel>>,
) -> impl Future<Item = Json<bool>, Error = Error> {
    Box::new(
        STORAGE
            .get_channel(channel.from_addr.clone())
            .and_then(move |mut channel_manager| {
                Ok(Json(channel_manager.check_proposal(&channel.data)?))
            })
            .responder(),
    )
}

pub fn channel_created_endpoint(
    channel: Json<NetworkRequest<Channel>>,
) -> impl Future<Item = Json<()>, Error = Error> {
    Box::new(
        STORAGE
            .get_channel(channel.from_addr.clone())
            .and_then(move |mut channel_manager| {
                channel_manager.channel_created(&channel.data, CRYPTO.own_eth_addr())?;
                Ok(Json(()))
            })
            .responder(),
    )
}
