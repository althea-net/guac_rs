use actix_web::client;
use actix_web::client::ClientResponse;
use actix_web::client::Connection;
use actix_web::HttpMessage;
use clarity::{Address, Signature};
use failure::Error;
use futures::{future, Future};
use guac_core::types::{NewChannelTx, ReDrawTx, UpdateTx};
use guac_core::CounterpartyApi;
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

pub struct CounterpartyClient;

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

impl CounterpartyApi for CounterpartyClient {
    fn propose_channel(
        &self,
        from_address: Address,
        to_url: String,
        new_channel_tx: NewChannelTx,
    ) -> Box<Future<Item = Signature, Error = Error>> {
        let to_url: Result<SocketAddr, std::net::AddrParseError> = to_url.parse();
        let to_url: SocketAddr = try_future_box!(to_url);
        // Prepare an endpoint for sending a proposal
        let endpoint = format!("http://[{}]:{}/propose_channel", to_url.ip(), to_url.port());
        // Connect to remote server
        let stream = TcpStream::connect(&to_url);

        Box::new(stream.from_err().and_then(move |stream| {
            client::post(&endpoint)
                .with_connection(Connection::from_stream(stream))
                .json((from_address, new_channel_tx))
                .unwrap()
                .send()
                .from_err()
                .and_then(verify_client_error)
                .and_then(move |response| {
                    response
                        .json()
                        .from_err()
                        .and_then(move |res: Signature| Ok(res))
                })
        })) as Box<Future<Item = Signature, Error = Error>>
    }

    fn propose_re_draw(
        &self,
        from_address: Address,
        to_url: String,
        re_draw_tx: ReDrawTx,
    ) -> Box<Future<Item = Signature, Error = Error>> {
        let to_url: Result<SocketAddr, std::net::AddrParseError> = to_url.parse();
        let to_url: SocketAddr = try_future_box!(to_url);
        // Prepare an endpoint for sending a proposal
        let endpoint = format!("http://[{}]:{}/propose_re_draw", to_url.ip(), to_url.port());
        // Connect to remote server
        let stream = TcpStream::connect(&to_url);

        Box::new(stream.from_err().and_then(move |stream| {
            client::post(&endpoint)
                .with_connection(Connection::from_stream(stream))
                .json((from_address, re_draw_tx))
                .unwrap()
                .send()
                .from_err()
                .and_then(verify_client_error)
                .and_then(move |response| {
                    response
                        .json()
                        .from_err()
                        .and_then(move |res: Signature| Ok(res))
                })
        })) as Box<Future<Item = Signature, Error = Error>>
    }

    fn notify_channel_opened(
        &self,
        from_address: Address,
        to_url: String,
    ) -> Box<Future<Item = (), Error = Error>> {
        let to_url: Result<SocketAddr, std::net::AddrParseError> = to_url.parse();
        let to_url: SocketAddr = try_future_box!(to_url);
        // Prepare an endpoint for sending a proposal
        let endpoint = format!(
            "http://[{}]:{}/notify_channel_opened",
            to_url.ip(),
            to_url.port()
        );
        // Connect to remote server
        let stream = TcpStream::connect(&to_url);

        Box::new(stream.from_err().and_then(move |stream| {
            client::post(&endpoint)
                .with_connection(Connection::from_stream(stream))
                .json(from_address)
                .unwrap()
                .send()
                .from_err()
                .and_then(verify_client_error)
                .and_then(|_| Ok(()))
        })) as Box<Future<Item = (), Error = Error>>
    }

    fn notify_re_draw(
        &self,
        from_address: Address,
        to_url: String,
    ) -> Box<Future<Item = (), Error = Error>> {
        let to_url: Result<SocketAddr, std::net::AddrParseError> = to_url.parse();
        let to_url: SocketAddr = try_future_box!(to_url);
        // Prepare an endpoint for sending a proposal
        let endpoint = format!("http://[{}]:{}/notify_re_draw", to_url.ip(), to_url.port());
        // Connect to remote server
        let stream = TcpStream::connect(&to_url);

        Box::new(stream.from_err().and_then(move |stream| {
            client::post(&endpoint)
                .with_connection(Connection::from_stream(stream))
                .json(from_address)
                .unwrap()
                .send()
                .from_err()
                .and_then(verify_client_error)
                .and_then(|_| Ok(()))
        })) as Box<Future<Item = (), Error = Error>>
    }

    fn receive_payment(
        &self,
        from_address: Address,
        to_url: String,
        update_tx: UpdateTx,
    ) -> Box<Future<Item = (), Error = Error>> {
        let to_url: Result<SocketAddr, std::net::AddrParseError> = to_url.parse();
        let to_url: SocketAddr = try_future_box!(to_url);
        // Prepare an endpoint for sending a proposal
        let endpoint = format!("http://[{}]:{}/receive_payment", to_url.ip(), to_url.port());
        // Connect to remote server
        let stream = TcpStream::connect(&to_url);

        Box::new(stream.from_err().and_then(move |stream| {
            client::post(&endpoint)
                .with_connection(Connection::from_stream(stream))
                .json((from_address, update_tx))
                .unwrap()
                .send()
                .from_err()
                .and_then(verify_client_error)
                .and_then(move |_| Ok(()))
        })) as Box<Future<Item = (), Error = Error>>
    }
}
