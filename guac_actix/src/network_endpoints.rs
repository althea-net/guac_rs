use actix_web::client;
use actix_web::AsyncResponder;
use actix_web::HttpMessage;
use actix_web::Json;

use guac_core::channel_client::types::{Channel, UpdateTx};
use guac_core::CRYPTO;
use guac_core::STORAGE;

use failure::Error;
use futures;
use futures::Future;

use NetworkRequest;

use althea_types::Bytes32;
use guac_core::counterparty::Counterparty;

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
