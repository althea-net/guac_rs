use actix_web::client;
use actix_web::AsyncResponder;
use actix_web::HttpMessage;
use actix_web::Json;
use channel_client::types::UpdateTx;
use failure::Error;
use futures;
use futures::Future;

use althea_types::Bytes32;
use counterparty::Counterparty;
use STORAGE;

pub fn update(update: Json<UpdateTx>) -> Box<Future<Item = Json<UpdateTx>, Error = Error>> {
    Box::new(
        STORAGE
            .get_data(update.channel_id.clone())
            .and_then(|mut channel_manager| {
                channel_manager.rec_payment(update.into_inner())?;
                Ok(Json(channel_manager.create_payment()?))
            })
            .responder(),
    )
}

pub fn send_payment(channel_id: Bytes32) -> impl Future<Item = (), Error = Error> {
    STORAGE
        .get_data(channel_id.clone())
        .and_then(move |mut channel_manager| {
            let sent_update = channel_manager.create_payment().unwrap();
            client::post(&format!("{}/update", channel_manager.counterparty.url))
                .json(sent_update.clone())
                .unwrap()
                .send()
                .from_err()
                .and_then( move |response| {
                    response
                        .json()
                        .from_err()
                        .and_then(move |res_update: UpdateTx| {
                            STORAGE.get_data(channel_id.clone()).and_then(
                                move |mut channel_manager| {
                                    channel_manager
                                        .rec_updated_state(sent_update.clone(), res_update)
                                },
                            )
                        })
                })
                .from_err()
        })
}
