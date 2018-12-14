use actix_web::http::Method;
use actix_web::*;

use clarity::Address;
use guac_core::channel_client::types::{NewChannelTx, ReDrawTx, UpdateTx};
use guac_core::Guac;
use guac_core::TransportProtocol;

use futures::Future;

pub fn init_server(port: u16, guac: Guac) {
    server::new(move || {
        App::with_state(guac.clone())
            .resource("/propose_channel", |r| {
                r.method(Method::POST).with_async(
                    move |(req, body): (HttpRequest<Guac>, Json<(Address, NewChannelTx)>)| {
                        let body = body.clone();
                        req.state()
                            .propose_channel(body.0, String::default(), body.1)
                            .and_then(move |res| Ok(Json(res)))
                            .responder()
                    },
                )
            })
            .resource("/propose_re_draw", |r| {
                r.method(Method::POST).with_async(
                    move |(req, body): (HttpRequest<Guac>, Json<(Address, ReDrawTx)>)| {
                        let body = body.clone();
                        req.state()
                            .propose_re_draw(body.0, String::default(), body.1)
                            .and_then(move |res| Ok(Json(res)))
                            .responder()
                    },
                )
            })
            .resource("/notify_channel_opened", |r| {
                r.method(Method::POST).with_async(
                    move |(req, body): (HttpRequest<Guac>, Json<Address>)| {
                        let body = body.clone();
                        req.state()
                            .notify_channel_opened(body, String::default())
                            .and_then(move |res| Ok(Json(res)))
                            .responder()
                    },
                )
            })
            .resource("/notify_re_draw", |r| {
                r.method(Method::POST).with_async(
                    move |(req, body): (HttpRequest<Guac>, Json<Address>)| {
                        let body = body.clone();
                        req.state()
                            .notify_re_draw(body, String::default())
                            .and_then(move |res| Ok(Json(res)))
                            .responder()
                    },
                )
            })
            .resource("/receive_payment", |r| {
                r.method(Method::POST).with_async(
                    move |(req, body): (HttpRequest<Guac>, Json<(Address, UpdateTx)>)| {
                        let body = body.clone();
                        req.state()
                            .receive_payment(body.0, String::default(), body.1)
                            .and_then(move |res| Ok(Json(res)))
                            .responder()
                    },
                )
            })
    })
    .bind(&format!("[::0]:{}", port))
    .unwrap()
    .start();
}
