use actix_web::http::Method;
use actix_web::*;

use clarity::{Address, PrivateKey, Signature};
use guac_core::channel_client::types::{Channel, NewChannelTx, ReDrawTx, UpdateTx};
use guac_core::Guac;
use guac_core::TransportProtocol;

use failure::Error;

use futures::Future;
use std::net::SocketAddr;
use std::sync::Arc;

pub fn init_server(port: u16, guac: Guac) {
    server::new(move || {
        // let guac_2 = guac.clone();
        // let guac_3 = guac.clone();
        // let guac_4 = guac.clone();
        // let guac_5 = guac.clone();
        App::new()
            .resource("/propose_channel", |r| {
                let guac = guac.clone();
                r.method(Method::POST)
                    .with_async(move |req: Json<(Address, NewChannelTx)>| {
                        let req = req.clone();
                        guac.propose_channel(req.0, String::default(), req.1)
                            .and_then(move |res| Ok(Json(res)))
                            .responder()
                    })
            })
            .resource("/propose_re_draw", |r| {
                let guac = guac.clone();
                r.method(Method::POST)
                    .with_async(move |req: Json<(Address, ReDrawTx)>| {
                        let req = req.clone();
                        guac.propose_re_draw(req.0, String::default(), req.1)
                            .and_then(move |res| Ok(Json(res)))
                            .responder()
                    })
            })
            .resource("/notify_channel_opened", |r| {
                let guac = guac.clone();
                r.method(Method::POST)
                    .with_async(move |req: Json<(Address)>| {
                        let req = req.clone();
                        guac.notify_channel_opened(req, String::default())
                            .and_then(move |res| Ok(Json(res)))
                            .responder()
                    })
            })
            .resource("/notify_re_draw", |r| {
                let guac = guac.clone();
                r.method(Method::POST)
                    .with_async(move |req: Json<(Address)>| {
                        let req = req.clone();
                        guac.notify_re_draw(req, String::default())
                            .and_then(move |res| Ok(Json(res)))
                            .responder()
                    })
            })
            .resource("/receive_payment", |r| {
                let guac = guac.clone();
                r.method(Method::POST)
                    .with_async(move |req: Json<(Address, UpdateTx)>| {
                        let req = req.clone();
                        guac.receive_payment(req.0, String::default(), req.1)
                            .and_then(move |res| Ok(Json(res)))
                            .responder()
                    })
            })
    })
    .bind(&format!("[::0]:{}", port))
    .unwrap()
    .start();
}
