use channel_client::types::{Channel, UpdateTx};
use channel_client::ChannelManager;
use clarity::Address;
use counterparty::Counterparty;
use crypto::CryptoService;
use failure::Error;
use futures::Future;
use {CRYPTO, STORAGE};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkRequest<T> {
    pub from_addr: Address,
    pub data: T,
}

impl<T> NetworkRequest<T> {
    pub fn wrap(data: T) -> NetworkRequest<T> {
        NetworkRequest {
            from_addr: CRYPTO.own_eth_addr(),
            data,
        }
    }
}

pub fn update(update: NetworkRequest<UpdateTx>) -> impl Future<Item = UpdateTx, Error = Error> {
    STORAGE
        .get_channel(update.from_addr)
        .and_then(move |mut channel_manager| {
            channel_manager.received_payment(&update.data)?;
            channel_manager.create_payment()
        })
}

pub fn propose_channel(
    to_url: String,
    channel: NetworkRequest<Channel>,
) -> impl Future<Item = bool, Error = Error> {
    let counterparty = Counterparty {
        address: channel.from_addr,
        url: to_url,
    };
    trace!("inserting state {:?}", counterparty);
    STORAGE
        .init_data(counterparty, ChannelManager::New)
        .then(|_| {
            STORAGE
                .get_channel(channel.from_addr)
                .and_then(move |mut channel_manager| channel_manager.check_proposal(&channel.data))
        })
}

pub fn channel_created(
    channel: NetworkRequest<Channel>,
) -> impl Future<Item = bool, Error = Error> {
    STORAGE
        .get_channel(channel.from_addr)
        .and_then(move |mut channel_manager| {
            channel_manager.channel_created(&channel.data, CRYPTO.own_eth_addr())?;
            Ok(true)
        })
}

pub fn channel_joined(channel: NetworkRequest<Channel>) -> impl Future<Item = bool, Error = Error> {
    STORAGE
        .get_channel(channel.from_addr)
        .and_then(move |mut channel_manager| {
            channel_manager.channel_joined(&channel.data)?;
            Ok(true)
        })
}
