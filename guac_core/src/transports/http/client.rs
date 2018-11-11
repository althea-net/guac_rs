use actix_web::client;
use actix_web::client::Connection;
use actix_web::HttpMessage;
use channel_client::types::{Channel, UpdateTx};
use failure::Error;
use futures::Future;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use transport_protocol::TransportProtocol;
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

impl TransportProtocol for HTTPTransportClient {
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
                    response
                        .json()
                        .from_err()
                        .and_then(move |res_update: UpdateTx| Ok(res_update))
                })
        }))
    }
}
