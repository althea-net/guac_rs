use actix_web::client;
use actix_web::client::ClientResponse;
use actix_web::client::Connection;
use actix_web::HttpMessage;
use clarity::{Address, Signature};
use failure::Error;
use futures::{future, Future};
use guac_core::types::{Counterparty, NewChannelTx, ReDrawTx, UpdateTx};
use guac_core::BountyHunterApi;
use num256::Uint256;
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::net::TcpStream;

macro_rules! try_future_box {
    ($expression:expr) => {
        match $expression {
            Err(err) => {
                return Box::new(future::err(err.into())) as Box<Future<Item = _, Error = Error>>;
            }
            Ok(value) => value,
        }
    };
}

#[derive(Clone)]
pub struct BountyHunterClient {
    pub bounty_hunter_url: String,
}

/// Verifies if the response from server is correct by checking status code.Client
///
/// Implementation of this is very simplified and all responses are expected to have HTTP 200 OK
/// response.
fn verify_client_error(
    response: ClientResponse,
) -> Box<Future<Item = ClientResponse, Error = Error>> {
    if response.status() != 200 {
        return Box::new(
            response.body().from_err().and_then(move |bod| {
                Err(format_err!("HTTP error {}: {:?}", response.status(), bod))
            }),
        );
    }
    Box::new(future::ok(response))
}

impl BountyHunterApi for BountyHunterClient {
    fn get_counterparties(
        &self,
        my_address: Address,
    ) -> Box<Future<Item = Option<HashMap<Address, Counterparty>>, Error = Error>> {
        let to_url: Result<SocketAddr, std::net::AddrParseError> = self.bounty_hunter_url.parse();
        let to_url: SocketAddr = try_future_box!(to_url);
        // Prepare an endpoint for sending a proposal
        let endpoint = format!(
            "http://[{}]:{}/get_counterparties",
            to_url.ip(),
            to_url.port()
        );
        // Connect to remote server
        let stream = TcpStream::connect(&to_url);

        Box::new(stream.from_err().and_then(move |stream| {
            client::post(&endpoint)
                .with_connection(Connection::from_stream(stream))
                .json(my_address)
                .expect("json parsing error")
                .send()
                .from_err()
                .and_then(verify_client_error)
                .and_then(move |response| {
                    response
                        .json()
                        .from_err()
                        .and_then(move |res: Option<HashMap<Address, Counterparty>>| Ok(res))
                })
        })) as Box<Future<Item = Option<HashMap<Address, Counterparty>>, Error = Error>>
    }
    fn set_counterparty(
        &self,
        my_address: Address,
        counterparty: Counterparty,
    ) -> Box<Future<Item = (), Error = Error>> {
        let to_url: Result<SocketAddr, std::net::AddrParseError> = self.bounty_hunter_url.parse();
        let to_url: SocketAddr = try_future_box!(to_url);
        // Prepare an endpoint for sending a proposal
        let endpoint = format!(
            "http://[{}]:{}/set_counterparty",
            to_url.ip(),
            to_url.port()
        );
        // Connect to remote server
        let stream = TcpStream::connect(&to_url);

        Box::new(stream.from_err().and_then(move |stream| {
            client::post(&endpoint)
                .with_connection(Connection::from_stream(stream))
                .json((my_address, counterparty))
                .expect("json parsing error")
                .send()
                .from_err()
                .and_then(verify_client_error)
                .and_then(move |response| {
                    response.json().from_err().and_then(move |res: ()| Ok(res))
                })
        })) as Box<Future<Item = (), Error = Error>>
    }
}
