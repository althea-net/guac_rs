use channel_client::types::{Channel, UpdateTx};
use failure::Error;
use futures::Future;

/// Defines a functionality of a transport protocol.
///
/// Its called a "protocol" because it provides a set of methods that both client and server should implement.
/// For example a server could keep track of channels, and client would issue HTTP requests.
pub trait CounterpartyApi {
    /// Send a proposal to other party
    fn send_proposal_request(&self, channel: &Channel) -> Box<Future<Item = bool, Error = Error>>;
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
