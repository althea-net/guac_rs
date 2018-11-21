use channel_client::types::{Channel, ChannelStatus};
use clarity::Address;
use crypto::CryptoService;
use failure::Error;
use futures::future::ok;
use futures::Future;
use num256::Uint256;
use payment_contract::ChannelId;
use qutex::Qutex;
use rand;
use rand::prelude::*;
use rand::RngCore;
use std::collections::HashMap;
use storage::Storage;
use CRYPTO;

struct ChannelData {
    url: String,

    // TODO: Channel structure needs some changes but we can reuse it at this point
    channel: Channel,
}

/// A in-memory storage that stores data in
pub struct InMemoryStorage {
    channels: Qutex<HashMap<ChannelId, ChannelData>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            channels: Qutex::new(HashMap::new()),
        }
    }
}

impl Storage for InMemoryStorage {
    /// Registers new counterparty
    ///
    /// * `url` - Remote URL
    /// * `url` - Remote ETH address
    /// * `balance` - Our initial deposit
    fn register(
        &self,
        url: String,
        address: Address,
        balance: Uint256,
    ) -> Box<Future<Item = Channel, Error = Error>> {
        // rand::thread_rng()
        let channel_id: ChannelId = {
            let mut data: ChannelId = Default::default();
            rand::thread_rng().fill_bytes(&mut data);
            data
        };
        // TODO: Remove CRYPTO global context and wait for future
        let own_eth_addr = CRYPTO.own_eth_addr();

        // Prepare channel data
        let channel = Channel {
            channel_id: Some(channel_id.into()),
            address_a: own_eth_addr,
            address_b: address,
            channel_status: ChannelStatus::New,
            deposit_a: balance.clone(),
            deposit_b: 0u64.into(),
            challenge: 0u64.into(),
            nonce: 0u64.into(),
            close_time: 0u64.into(), // TODO: add expire + settling
            balance_a: balance,
            balance_b: 0u64.into(),
            is_a: true, // TODO: not necessary as addresses are supposed to be ordered?
        };
        let data = ChannelData {
            url,
            channel: channel.clone(),
        };

        Box::new(
            self.channels
                .clone()
                .lock()
                .and_then(move |mut channels| {
                    channels.insert(channel_id, data);
                    Ok(channel.clone())
                }).from_err(),
        )
    }
}

#[test]
fn register() {
    let channels = InMemoryStorage::new();
    channels
        .register(
            "42.42.42.42:4242".to_string(),
            Address::new(),
            123u64.into(),
        ).wait()
        .unwrap();
}
