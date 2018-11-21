use failure::Error;
use transport_protocol::{TransportFactory, TransportProtocol};
use transports::http::client::HTTPTransportClient;

pub struct HTTPTransportFactory {}

impl HTTPTransportFactory {
    pub fn new() -> Self {
        Self {}
    }
}

impl TransportFactory for HTTPTransportFactory {
    fn create_transport_protocol(&self, url: String) -> Result<Box<TransportProtocol>, Error> {
        Ok(Box::new(HTTPTransportClient::new(url)?))
    }
}
