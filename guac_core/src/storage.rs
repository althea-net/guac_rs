use clarity::Address;
use counterparty::Counterparty;
use failure::Error;

use futures;
use futures::future::join_all;
use futures::Future;

use crypto::CryptoService;
use CRYPTO;

use channel_client::types::Channel;
use num256::Uint256;
use qutex::{FutureGuard, Guard, QrwLock, Qutex};
use std::collections::HashMap;

/// A trait that describes a way to to manage channels.
///
/// This may allow multiple implementations such as in-memory storage,
/// or a storage thats backed by a bounty hunter. One could chain in-memory storage
/// with a BH storage to replicate local state with remote server for consistency.
pub trait Storage {
    /// Creates a new channel for given parameters of a counterparty.
    fn register(
        &self,
        url: String,
        address: Address,
        balance: Uint256,
    ) -> Box<Future<Item = Channel, Error = Error>>;
}
