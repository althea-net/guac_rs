#![cfg_attr(test, feature(proc_macro))]
//! Actix web diesel example
//!
//! Diesel does not support tokio, so we have to run it in separate threads.
//! Actix supports sync actors by default, so we going to create sync actor that use diesel.
//! Technically sync actors are worker style actors, multiple of them
//! can run in parallel and process messages from same queue.
extern crate actix;
extern crate actix_web;
#[macro_use]
extern crate diesel;
extern crate env_logger;
extern crate futures;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate althea_types;
extern crate base64;
#[macro_use]
extern crate failure;
extern crate num256;
extern crate serde_json;
extern crate tiny_keccak;
extern crate uuid;
extern crate web3;

#[cfg(test)]
extern crate mocktopus;

use actix::prelude::*;
use actix_web::{middleware, Application, AsyncResponder, Error, HttpRequest, HttpResponse,
                HttpServer, Method};

use diesel::prelude::*;
use futures::future::Future;

mod crypto;
mod db;
mod logic;
mod models;
mod schema;
mod types;

use db::{CreateUser, DbExecutor};

/// State with DbExecutor address
struct State {
    db: Addr<Syn, DbExecutor>,
}

/// Async request handler
fn index(req: HttpRequest<State>) -> Box<Future<Item = HttpResponse, Error = Error>> {
    let name = &req.match_info()["name"];

    // send async `CreateUser` message to a `DbExecutor`
    req.state()
        .db
        .send(CreateUser {
            name: name.to_owned(),
        })
        .from_err()
        .and_then(|res| match res {
            Ok(user) => Ok(HttpResponse::Ok().json(user)?),
            Err(_) => Ok(HttpResponse::InternalServerError().into()),
        })
        .responder()
}

// fn propose_channel() -> Box<Future<Item = HttpResponse, Error = Error>> {}

fn main() {
    ::std::env::set_var("RUST_LOG", "actix_web=info");
    let _ = env_logger::init();
    let sys = actix::System::new("diesel-example");

    // Start 3 db executor actors
    let addr = SyncArbiter::start(3, || {
        DbExecutor(SqliteConnection::establish(":memory").unwrap())
    });

    // Start http server
    let _addr = HttpServer::new(move || {
        Application::with_state(State{db: addr.clone()})
            // enable logger
            .middleware(middleware::Logger::default())
            .resource("/{name}", |r| r.method(Method::GET).a(index))
    }).bind("127.0.0.1:8080")
        .unwrap()
        .start();

    println!("Started http server: 127.0.0.1:8080");
    let _ = sys.run();
}
