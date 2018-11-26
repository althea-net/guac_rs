use clarity::Address;
use clarity::Signature;
use failure::Error;
use futures::future::ok;
use futures::Future;
use futures::IntoFuture;
use num256::Uint256;

/// An alias for a channel ID in a raw bytes form
pub type ChannelId = [u8; 32];

pub trait PaymentContract {
    fn new_channel(
        &self,
        address0: Address,
        address1: Address,
        balance0: Uint256,
        balance1: Uint256,
        signature0: Signature,
        signature1: Signature,
        expiration: Uint256,
        settling_period: Uint256,
    ) -> Box<Future<Item = ChannelId, Error = Error>>;
    fn update_state(
        &self,
        channel_id: ChannelId,
        channel_nonce: Uint256,
        balance_a: Uint256,
        balance_b: Uint256,
        sig_a: Signature,
        sig_b: Signature,
    ) -> Box<Future<Item = (), Error = Error>>;
    fn start_challenge(&self, channel_id: ChannelId) -> Box<Future<Item = (), Error = Error>>;
    fn close_channel(&self, channel_id: ChannelId) -> Box<Future<Item = (), Error = Error>>;

    fn quick_deposit(&self, value: Uint256) -> Box<Future<Item = (), Error = Error>>;
}
