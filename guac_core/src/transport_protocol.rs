use channel_client::types::{Channel, UpdateTx};
use clarity::Signature;
use failure::Error;
use futures::Future;

/// Defines a functionality of a transport protocol.
///
/// Its called a "protocol" because it provides a set of methods that both client and server should implement.
/// For example a server could keep track of channels, and client would issue HTTP requests.
pub trait TransportProtocol {
    /// Send a proposal to other party and returns a valid Signature
    fn send_proposal_request(
        &self,
        channel: &Channel,
    ) -> Box<Future<Item = Signature, Error = Error>>;
    /// Sends a channel created request
    fn send_channel_created_request(
        &self,
        channel: &Channel,
    ) -> Box<Future<Item = (), Error = Error>>;
    /// Send channel update
    fn send_channel_update(
        &self,
        update_tx: &UpdateTx,
    ) -> Box<Future<Item = UpdateTx, Error = Error>>;
    /// Send channel joined
    fn send_channel_joined(&self, channel: &Channel) -> Box<Future<Item = (), Error = Error>>;
}

/// Defines a functionality of a transport factory.
///
/// A transport factory provides instances of TransportProtocol instances for
/// a given URL.
///
/// One usage example of such trait would be to implement a factory that
/// would spawn instances of client TransportProtocol with an URL passed
/// already. A transport factory could select a specific transport implementation
/// given the url.
pub trait TransportFactory {
    /// Creates a transport for a given URL.
    fn create_transport_protocol(&self, url: String) -> Result<Box<TransportProtocol>, Error>;
}
