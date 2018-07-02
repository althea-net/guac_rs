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

pub fn send_payment(counterparty: Counterparty) -> impl Future<Item = (), Error = Error> {
    STORAGE
        .get_channel(counterparty.address)
        .and_then(move |mut channel_manager| {
            let sent_update = channel_manager.create_payment().unwrap();
            client::post(&format!("{}/update", counterparty.url))
                .json(sent_update)
                .unwrap()
                .send()
                .from_err()
                .and_then(move |response| {
                    response
                        .json()
                        .from_err()
                        .and_then(move |res_update: UpdateTx| {
                            channel_manager.rec_updated_state(&res_update)
                        })
                })
                .from_err()
        })
}
