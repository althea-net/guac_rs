extern crate actix;
extern crate actix_web;
extern crate althea_types;
extern crate bytes;
extern crate clarity;
#[macro_use]
extern crate failure;
extern crate futures;
extern crate guac_core;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate num256;
extern crate qutex;
extern crate serde;
extern crate serde_json;
extern crate tokio;

use actix::prelude::*;
use actix_web::*;
use althea_types::PaymentTx;
use failure::Error;
use futures::Future;

use guac_core::channel_client::types::UpdateTx;
use guac_core::Guac;

mod network_endpoints;
mod network_requests;

pub use network_endpoints::init_server;

use althea_types::Identity;
use clarity::Address;
use num256::Uint256;
use std::net::{IpAddr, Ipv6Addr};
use std::ops::Add;
