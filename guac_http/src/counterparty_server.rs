use actix_web::http::Method;
use actix_web::*;

use clarity::Address;
use failure::Error;
use guac_core::types::{NewChannelTx, ReDrawTx, UpdateTx};
use guac_core::CounterpartyApi;
use guac_core::Guac;
use guac_core::GuacError;

use futures::future;
use futures::Future;
// .map_err(|err| match err.downcast::<GuacError::Forbidden>() {
//     Ok(forbidden) => ok(HttpResponse::Forbidden().body(forbidden.message)),
//     Err => ok(HttpResponse::InternalServerError().finish()),
// })

fn convert_error(err: Error) -> HttpResponse {
    match err.downcast::<GuacError>() {
        Ok(guac_err) => match guac_err {
            GuacError::Forbidden { message } => HttpResponse::Forbidden().body(message),
            GuacError::UpdateTooOld { correct_seq } => HttpResponse::Conflict().json(correct_seq),
            _ => HttpResponse::InternalServerError().finish(),
        },
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

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
                            .then(|res| match res {
                                Ok(res) => future::ok::<HttpResponse, failure::Error>(
                                    HttpResponse::Ok().json(res),
                                ),
                                Err(err) => future::ok(convert_error(err)),
                            })
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
