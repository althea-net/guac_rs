use clarity::Address;
use counterparty::Counterparty;
use failure::Error;

use futures;
use futures::future::join_all;
use futures::Future;

use crypto::CryptoService;
use CRYPTO;

use channel_client::types::{Channel, ChannelState};
use num256::Uint256;
use qutex::{FutureGuard, Guard, QrwLock, Qutex};
use std::collections::HashMap;

/// A trait that describes a way to to manage channels.
///
/// This may allow multiple implementations such as in-memory storage,
/// or a storage thats backed by a bounty hunter. One could chain in-memory storage
/// with a BH storage to replicate local state with remote server for consistency.
pub trait ChannelStorage {
    /// Creates a new channel for given parameters of a counterparty.
    fn register_channel(
        &self,
        url: String,
        address0: Address,
        address1: Address,
        balance0: Uint256,
        balance1: Uint256,
    ) -> Box<Future<Item = Channel, Error = Error>>;
    /// Get channel struct for a given channel ID.
    ///
    /// - `channel_id` - A valid channel ID
    fn get_channel(&self, state: ChannelState) -> Box<Future<Item = Channel, Error = Error>>;
    /// Update a channel by its ID
    fn update_channel(
        &self,
        state: ChannelState,
        channel: Channel,
    ) -> Box<Future<Item = (), Error = Error>>;
}
