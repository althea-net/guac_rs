use channel_client::types::{Channel, UpdateTx};
use channel_client::ChannelManager;
use clarity::Address;
use counterparty::Counterparty;
use crypto::CryptoService;
use failure::Error;
use futures::Future;
use CRYPTO;

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

pub fn update(update: NetworkRequest<UpdateTx>) -> Box<Future<Item = UpdateTx, Error = Error>> {
    unimplemented!()
}

pub fn propose_channel(
    to_url: String,
    channel: NetworkRequest<Channel>,
) -> Box<Future<Item = bool, Error = Error>> {
    unimplemented!()
}

pub fn channel_created(
    channel: NetworkRequest<Channel>,
) -> Box<Future<Item = bool, Error = Error>> {
    unimplemented!()
}

pub fn channel_joined(channel: NetworkRequest<Channel>) -> Box<Future<Item = bool, Error = Error>> {
    unimplemented!()
}
