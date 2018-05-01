extern crate rand;

use althea_types::{Bytes32, EthAddress, EthPrivateKey, EthSignature};
// use crypto::Crypto;
use failure::{Error, SyncFailure};
use num256::Uint256;
use types::{Channel, ChannelStatus, Counterparty, NewChannelTx, UpdateTx};
// use ethkey::{sign, Message, Secret};
use futures::{future, Future};
use std::cell::RefCell;
use web3::contract::{Contract as Ctr, Options};
use web3::transports::http::Http;

#[cfg(test)]
use mocktopus::macros::*;

#[derive(Debug, Fail)]
enum CallerServerError {
    #[fail(display = "Could not find counterparty")]
    CounterPartyNotFound {},
    #[fail(display = "Could not find channel")]
    ChannelNotFound {},
}

mod storage {
    use super::*;

    pub fn new_channel(channel: &Channel) -> Result<(), Error> {
        Ok(())
    }
    pub fn save_channel(channel: &Channel) -> Result<(), Error> {
        Ok(())
    }
    pub fn save_update(update: &UpdateTx) -> Result<(), Error> {
        Ok(())
    }
    pub fn get_counterparty_by_address(
        eth_address: &EthAddress,
    ) -> Result<Option<Counterparty>, Error> {
        Ok(Some(Counterparty {
            address: *eth_address,
            url: String::from(""),
        }))
    }
    pub fn get_channel_of_counterparty(
        counterparty: &Counterparty,
    ) -> Result<Option<Channel>, Error> {
        Ok(Some(Channel {
            channel_id: Bytes32([0; 32]),
            address_a: EthAddress([0; 20]),
            address_b: EthAddress([0; 20]),
            channel_status: ChannelStatus::Open,
            deposit_a: 0.into(),
            deposit_b: 0.into(),
            challenge: 0.into(),
            nonce: 0.into(),
            close_time: 0.into(),
            balance_a: 0.into(),
            balance_b: 0.into(),
            is_a: true,
        }))
    }
}

mod counterparty_client {
    use super::*;

    pub fn make_payment(
        their_url: &str,
        update_tx: &UpdateTx,
    ) -> Box<Future<Item = EthSignature, Error = Error>> {
        Box::new(future::ok(EthSignature([0; 65])))
    }
}

mod crypto {
    use super::*;

    pub fn hash_bytes(bytes: &[&[u8]]) -> Bytes32 {
        Bytes32([0; 32])
    }

    pub fn eth_sign(key: &EthPrivateKey, input: &Bytes32) -> EthSignature {
        EthSignature([0; 65])
    }
}

struct Contract {
    contract: Ctr<Http>,
}

impl Contract {
    pub fn open_channel(channel_id: Bytes32, counterparty_address: EthAddress) {}
}

pub struct CallerServer {
    pub contract: Contract<Http>,
    pub my_eth_address: EthAddress,
    pub challenge_length: Uint256,
}

impl CallerServer {
    pub fn open_channel(
        &'static self,
        amount: Uint256,
        their_eth_address: EthAddress,
    ) -> Box<Future<Item = (), Error = Error>> {
        let channel_id = Bytes32([0; 32]);
        Box::new(
            self.contract
                .call_with_confirmations(
                    "openChannel".into(),
                    (channel_id.0),
                    EthAddress([0; 20]).0.into(),
                    Options::with(|options| ()),
                    1u8.into(),
                )
                .map_err(SyncFailure::new)
                .from_err()
                .and_then(move |_| {
                    let channel = Channel {
                        channel_id,
                        address_a: self.my_eth_address.clone(),
                        address_b: their_eth_address,
                        channel_status: ChannelStatus::Open,
                        deposit_a: amount,
                        deposit_b: 0.into(),
                        challenge: self.challenge_length.clone(),
                        nonce: 0.into(),
                        close_time: 0.into(),
                        balance_a: 0.into(),
                        balance_b: 0.into(),
                        is_a: true,
                    };
                    match storage::new_channel(&channel) {
                        Err(err) => return Err(err),
                        _ => return Ok(()),
                    };
                }),
        )
    }

    pub fn join_channel(&self, channel_Id: Bytes32, amount: Uint256) -> Result<(), Error> {
        // Call eth somehow
        Ok(())
    }

    pub fn make_payment(
        &'static self,
        their_url: &str,
        their_address: EthAddress,
        amount: Uint256,
    ) -> Box<Future<Item = (), Error = Error>> {
        let counterparty = match storage::get_counterparty_by_address(&their_address) {
            Ok(Some(counterparty)) => counterparty,
            Ok(None) => {
                return Box::new(future::err(Error::from(
                    CallerServerError::CounterPartyNotFound {},
                )))
            }
            Err(err) => return Box::new(future::err(err)),
        };

        let mut channel = match storage::get_channel_of_counterparty(&counterparty) {
            Ok(Some(channel)) => channel,
            Ok(None) => {
                return Box::new(future::err(Error::from(
                    CallerServerError::ChannelNotFound {},
                )))
            }
            Err(err) => return Box::new(future::err(err)),
        };

        let my_balance = channel.get_my_balance();
        let their_balance = channel.get_their_balance();

        channel.nonce = channel.nonce + 1;

        channel.set_my_balance(&(my_balance - amount.clone()));
        channel.set_their_balance(&(their_balance + amount));

        let mut update_tx = UpdateTx {
            channel_id: channel.channel_id.clone(),
            nonce: channel.nonce.clone() + 1,
            balance_a: channel.balance_a.clone(),
            balance_b: channel.balance_b.clone(),
            signature_a: None,
            signature_b: None,
        };

        let fingerprint = crypto::hash_bytes(&[
            update_tx.channel_id.as_ref(),
            &update_tx.nonce.to_bytes_le(),
            &update_tx.balance_a.to_bytes_le(),
            &update_tx.balance_b.to_bytes_le(),
        ]);

        let my_sig = crypto::eth_sign(&EthPrivateKey([0; 64]), &fingerprint);

        update_tx.set_my_signature(channel.is_a, &my_sig);

        storage::save_channel(&channel);
        storage::save_update(&update_tx);

        Box::new(
            counterparty_client::make_payment(their_url, &update_tx)
                .from_err()
                .and_then(move |their_signature| {
                    update_tx.set_their_signature(channel.is_a, &their_signature);
                    match storage::save_channel(&channel) {
                        Err(err) => return Err(err),
                        _ => (),
                    };
                    match storage::save_update(&update_tx) {
                        Err(err) => return Err(err),
                        _ => (),
                    };

                    Ok(())
                }),
        )
    }

    // pub fn close_channel (&self, their_address: EthAddress) -> Box<Future<Item = (), Error = Error>> {

    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mocktopus::mocking::*;

    #[test]
    fn happy_path() {}
}
