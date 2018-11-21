use channel_client::types::{Channel, ChannelStatus};
use clarity::Address;
use crypto::CryptoService;
use failure::Error;
use futures::future::ok;
use futures::Future;
use num256::Uint256;
use qutex::Qutex;
use rand;
use rand::prelude::*;
use rand::RngCore;
use std::collections::HashMap;
use storage::Storage;

/// A in-memory storage that stores data in
pub struct InMemoryStorage {
    channels: Qutex<HashMap<Uint256, Channel>>,
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
        address0: Address,
        address1: Address,
        balance0: Uint256,
        balance1: Uint256,
    ) -> Box<Future<Item = Channel, Error = Error>> {
        // rand::thread_rng()
        let channel_id: Uint256 = {
            let mut data: [u8; 32] = Default::default();
            rand::thread_rng().fill_bytes(&mut data);
            data.into()
        };

        // Prepare channel data
        let channel = Channel {
            channel_id: Some(channel_id.clone().into()),
            address_a: address0,
            address_b: address1,
            channel_status: ChannelStatus::New,
            deposit_a: balance0.clone(),
            deposit_b: balance1.clone(),
            challenge: 0u64.into(),
            nonce: 0u64.into(),
            close_time: 0u64.into(), // TODO: add expire + settling
            balance_a: balance0,
            balance_b: balance1,
            is_a: true, // TODO: not necessary as addresses are supposed to be ordered?
            url,
        };

        Box::new(
            self.channels
                .clone()
                .lock()
                .and_then(move |mut channels| {
                    channels.insert(channel_id.clone(), channel.clone());
                    Ok(channel)
                }).from_err(),
        )
    }
    fn get_channel(&self, channel_id: Uint256) -> Box<Future<Item = Channel, Error = Error>> {
        Box::new(
            self.channels
                .clone()
                .lock()
                .from_err()
                .and_then(move |channels| {
                    channels
                        .get(&channel_id)
                        .ok_or(format_err!("Unable to find channel {:x?}", &channel_id))
                        .map(move |value| value.clone())
                }),
        )
    }

    fn update_channel(
        &self,
        channel_id: Uint256,
        channel: Channel,
    ) -> Box<Future<Item = (), Error = Error>> {
        Box::new(
            self.channels
                .clone()
                .lock()
                .from_err()
                .and_then(move |mut channels| {
                    let mut entry = channels
                        .get_mut(&channel_id)
                        .ok_or(format_err!("Unable to find channel {:x?}", &channel_id))?;
                    *entry = channel;
                    Ok(())
                }),
        )
    }
}

#[test]
fn register() {
    let channels = InMemoryStorage::new();
    let channel = channels
        .register(
            "42.42.42.42:4242".to_string(),
            "0x0000000000000000000000000000000000000001"
                .parse()
                .unwrap(),
            "0x0000000000000000000000000000000000000002"
                .parse()
                .unwrap(),
            123u64.into(),
            0u64.into(),
        ).wait()
        .unwrap();

    let mut stored_channel = channels
        .get_channel(channel.channel_id.clone().unwrap())
        .wait()
        .unwrap();
    assert_eq!(stored_channel.url, "42.42.42.42:4242");

    stored_channel.nonce += 1;

    channels
        .update_channel(channel.channel_id.clone().unwrap(), channel.clone())
        .wait()
        .unwrap();
}
