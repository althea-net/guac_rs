use actix_web::client;
use actix_web::client::ClientResponse;
use actix_web::client::Connection;
use actix_web::HttpMessage;
use channel_client::types::{Channel, UpdateTx};
use failure::Error;
use futures::Future;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use transport_protocol::CounterpartyApi;
use transports::http::network_request::NetworkRequest;
/// Represnetation of an transport client that works over HTTP.
///
/// Contains useful properties to make an HTTP request. One instance
/// is bound to single URL.
///
/// This URL will be used to query sub resources over the network. At
/// some point we might want to include a "API" root by convention here,
/// not necessarily a transport.
pub struct HTTPTransportClient {
    /// Base URL for destination.
    addr: SocketAddr,
}

impl HTTPTransportClient {
    pub fn new(url: String) -> Result<HTTPTransportClient, Error> {
        Ok(HTTPTransportClient { addr: url.parse()? })
    }
}

/// Verifies if the response from server is correct by checking status code.HTTPTransportClient
///
/// Implementation of this is very simplified and all responses are expected to have HTTP 200 OK
/// response.
fn verify_client_error(response: ClientResponse) -> Result<ClientResponse, Error> {
    if response.status() != 200 {
        return Err(format_err!(
            "Received client error from server: {}",
            response.status()
        ));
    }
    Ok(response)
}

impl CounterpartyApi for HTTPTransportClient {
    fn send_proposal_request(&self, channel: &Channel) -> Box<Future<Item = bool, Error = Error>> {
        trace!(
            "Send channel proposal request channel={:?} addr={}",
            channel.clone(),
            self.addr,
        );
        // Prepare an endpoint for sending a proposal
        let endpoint = format!("http://[{}]:{}/propose", self.addr.ip(), self.addr.port());
        // Connect to remote server
        let stream = TcpStream::connect(&self.addr);
        // Prepare a payload to be sent
        let payload = NetworkRequest::from_data(channel.clone());
        Box::new(stream.from_err().and_then(move |stream| {
            client::post(&endpoint)
                .with_connection(Connection::from_stream(stream))
                .json(payload)
                .unwrap()
                .send()
                .from_err()
                .and_then(verify_client_error)
                .and_then(move |response| {
                    response
                        .json()
                        .from_err()
                        .and_then(move |res: bool| Ok(res))
                })
        }))
    }
    /// Sends a channel created request
    fn send_channel_created_request(
        &self,
        channel: &Channel,
    ) -> Box<Future<Item = (), Error = Error>> {
        trace!(
            "Send created request channel={:?} addr={}",
            channel,
            self.addr
        );
        // Prepare URL for sending channel created notification
        let endpoint = format!(
            "http://[{}]:{}/channel_created",
            self.addr.ip(),
            self.addr.port()
        );
        // Create a connection to remote server
        let stream = TcpStream::connect(&self.addr);
        // Prepare a payload for sending
        let payload = NetworkRequest::from_data(channel.clone());
        // Process request
        Box::new(stream.from_err().and_then(move |stream| {
            client::post(&endpoint)
                .with_connection(Connection::from_stream(stream))
                .json(payload)
                .unwrap()
                .send()
                .from_err()
                .and_then(verify_client_error)
                .and_then(move |response| {
                    response.body().from_err().and_then(move |res| {
                        trace!("Channel created request returned {:?}", res);
                        Ok(())
                    })
                })
        }))
    }

    /// Send channel update
    fn send_channel_update(
        &self,
        update_tx: &UpdateTx,
    ) -> Box<Future<Item = UpdateTx, Error = Error>> {
        trace!(
            "Send channel update request update={:?} url={}",
            update_tx,
            self.addr,
        );
        let endpoint = format!("http://[{}]:{}/update", self.addr.ip(), self.addr.port());

        let stream = TcpStream::connect(&self.addr);

        let payload = NetworkRequest::from_data(update_tx.clone());

        Box::new(stream.from_err().and_then(move |stream| {
            client::post(&endpoint)
                .with_connection(Connection::from_stream(stream))
                .json(payload)
                .unwrap()
                .send()
                .from_err()
                .and_then(move |response| {
                    if response.status() != 200 {
                        return Err(format_err!(
                            "Received client error from server: {}",
                            response.status()
                        ));
                    }
                    Ok(response)
                })
                .and_then(move |response| {
                    response
                        .json()
                        .from_err()
                        .and_then(move |res_update: UpdateTx| Ok(res_update))
                })
        }))
    }

    // Send a channel joined to other party
    fn send_channel_joined(&self, channel: &Channel) -> Box<Future<Item = (), Error = Error>> {
        trace!(
            "Send channel joined request channel={:?} url={}",
            channel,
            self.addr,
        );
        // Prepare URL
        let endpoint = format!(
            "http://[{}]:{}/channel_joined",
            self.addr.ip(),
            self.addr.port()
        );
        // Make a payload
        let payload = NetworkRequest::from_data(channel.clone());
        // Connect to server
        Box::new(
            TcpStream::connect(&self.addr)
                .from_err()
                .and_then(move |stream| {
                    client::post(&endpoint)
                        .with_connection(Connection::from_stream(stream))
                        .json(payload)
                        .unwrap()
                        .send()
                        .from_err()
                        .and_then(move |response| {
                            if response.status() != 200 {
                                return Err(format_err!(
                                    "Received client error from server: {}",
                                    response.status()
                                ));
                            }
                            Ok(response)
                        })
                        .and_then(move |response| response.body().from_err().and_then(|_| Ok(())))
                }),
        )
    }
}

#[cfg(test)]
fn make_channel() -> Channel {
    Channel {
        channel_id: Some(42u64.into()),
        address_a: "0x0000000000000000000000000000000000000001"
            .parse()
            .unwrap(),
        address_b: "0x0000000000000000000000000000000000000002"
            .parse()
            .unwrap(),
        channel_status: ChannelStatus::Joined,
        deposit_a: 0u64.into(),
        deposit_b: 1u64.into(),
        challenge: 0u64.into(),
        nonce: 0u64.into(),
        close_time: 10u64.into(),
        balance_a: 0u64.into(),
        balance_b: 1u64.into(),
        is_a: true,
    }
}

#[test]
fn proposal() {
    use actix::{Arbiter, Handler, System};
    use mockito::mock;

    let _m = mock("POST", "/propose")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("true")
        .create();

    let client = HTTPTransportClient {
        addr: mockito::SERVER_ADDRESS
            .parse()
            .expect("Invalid mockito address"),
    };
    let channel = make_channel();
    let sys = System::new("test");
    Arbiter::spawn({
        client.send_proposal_request(&channel).then(|res| {
            assert_eq!(
                res.expect("Expected a valid bool response but got error instead"),
                true
            );
            System::current().stop();
            Ok(())
        })
    });
    sys.run();
}

#[test]
fn invalid_proposal() {
    use actix::{Arbiter, Handler, System};
    use mockito::mock;

    let _m = mock("POST", "/propose")
        .with_status(404)
        .with_header("content-type", "application/json")
        .create();

    let client = HTTPTransportClient {
        addr: mockito::SERVER_ADDRESS
            .parse()
            .expect("Invalid mockito address"),
    };
    let channel = make_channel();
    let sys = System::new("test");
    Arbiter::spawn({
        client.send_proposal_request(&channel).then(|res| {
            let err = res.expect_err("Expected an error but got a response instead");
            assert!(format!("{}", err).starts_with("Received client error from server: 404"));
            System::current().stop();
            Ok(())
        })
    });
    sys.run();
}

#[test]
fn channel_created() {
    use actix::{Arbiter, Handler, System};
    use mockito::mock;

    let _m = mock("POST", "/channel_created").with_status(200).create();

    let client = HTTPTransportClient {
        addr: mockito::SERVER_ADDRESS
            .parse()
            .expect("Invalid mockito address"),
    };
    let channel = make_channel();
    let sys = System::new("test");
    Arbiter::spawn({
        client.send_channel_created_request(&channel).then(|res| {
            res.expect("Expected a valid response but got error instead");
            System::current().stop();
            Ok(())
        })
    });
    sys.run();
}

#[test]
fn invalid_channel_created() {
    use actix::{Arbiter, Handler, System};
    use mockito::mock;

    let _m = mock("POST", "/channel_created").with_status(404).create();

    let client = HTTPTransportClient {
        addr: mockito::SERVER_ADDRESS
            .parse()
            .expect("Invalid mockito address"),
    };
    let channel = make_channel();
    let sys = System::new("test");
    Arbiter::spawn({
        client.send_channel_created_request(&channel).then(|res| {
            let err = res.expect_err("Expected an error but got valid error instead");
            assert!(format!("{}", err).starts_with("Received client error from server: 404"));
            System::current().stop();
            Ok(())
        })
    });
    sys.run();
}

#[test]
fn channel_joined() {
    use actix::{Arbiter, Handler, System};
    use mockito::mock;

    let _m = mock("POST", "/channel_joined").with_status(200).create();

    let client = HTTPTransportClient {
        addr: mockito::SERVER_ADDRESS
            .parse()
            .expect("Invalid mockito address"),
    };
    let channel = make_channel();
    let sys = System::new("test");
    Arbiter::spawn({
        client.send_channel_joined(&channel).then(|res| {
            res.expect("Expected a valid response but got error instead");
            System::current().stop();
            Ok(())
        })
    });
    sys.run();
}

#[test]
fn invalid_channel_joined() {
    use actix::{Arbiter, Handler, System};
    use mockito::mock;

    let _m = mock("POST", "/channel_joined").with_status(404).create();

    let client = HTTPTransportClient {
        addr: mockito::SERVER_ADDRESS
            .parse()
            .expect("Invalid mockito address"),
    };
    let channel = make_channel();
    let sys = System::new("test");
    Arbiter::spawn({
        client.send_channel_joined(&channel).then(|res| {
            let err = res.expect_err("Expected a valid response but got error instead");
            assert!(format!("{}", err).starts_with("Received client error from server: 404"));
            System::current().stop();
            Ok(())
        })
    });
    sys.run();
}

#[test]
fn update() {
    use actix::{Arbiter, Handler, System};
    use mockito::mock;
    use serde_json;

    let update_request = UpdateTx {
        channel_id: 1234u64.into(),
        nonce: 0u64.into(),
        balance_a: 100u64.into(),
        balance_b: 200u64.into(),
        signature_a: None,
        signature_b: None,
    };

    let _m = mock("POST", "/update")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&update_request).unwrap())
        .create();
    let client = HTTPTransportClient {
        addr: mockito::SERVER_ADDRESS
            .parse()
            .expect("Invalid mockito address"),
    };
    let channel = make_channel();
    let sys = System::new("test");
    Arbiter::spawn({
        client
            .send_channel_update(&update_request.clone())
            .then(|res| {
                let update = res.expect("Expected a valid response but got error instead");
                assert_eq!(
                    update,
                    UpdateTx {
                        channel_id: 1234u64.into(),
                        nonce: 0u64.into(),
                        balance_a: 100u64.into(),
                        balance_b: 200u64.into(),
                        signature_a: None,
                        signature_b: None,
                    }
                );
                System::current().stop();
                Ok(())
            })
    });
    sys.run();
}

#[test]
fn invalid_update() {
    use actix::{Arbiter, Handler, System};
    use mockito::mock;
    use serde_json;

    let update_request = UpdateTx {
        channel_id: 1234u64.into(),
        nonce: 0u64.into(),
        balance_a: 100u64.into(),
        balance_b: 200u64.into(),
        signature_a: None,
        signature_b: None,
    };

    let _m = mock("POST", "/update")
        .with_status(404)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&update_request).unwrap())
        .create();
    let client = HTTPTransportClient {
        addr: mockito::SERVER_ADDRESS
            .parse()
            .expect("Invalid mockito address"),
    };
    let channel = make_channel();
    let sys = System::new("test");
    Arbiter::spawn({
        client
            .send_channel_update(&update_request.clone())
            .then(|res| {
                let err = res.expect_err("Expected an error but got a valid response instead");
                assert!(format!("{}", err).starts_with("Received client error from server: 404"));
                System::current().stop();
                Ok(())
            })
    });
    sys.run();
}
