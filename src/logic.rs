extern crate rand;

use althea_types::{EthAddress, Bytes32, EthPrivateKey, EthSignature};
// use crypto::Crypto;
use failure::Error;
use num256::Uint256;
use types::{Channel, Counterparty, UpdateTx, NewChannelTx, ChannelStatus};
// use ethkey::{sign, Message, Secret};
use futures::{future, Future};

#[derive(Debug, Fail)]
enum CallerServerError {
  #[fail(display = "Could not find counterparty")]
  CounterPartyNotFound {},
  #[fail(display = "Could not find channel")]
  ChannelNotFound {},
}

pub trait Storage {
  fn new_channel(&self, channel: Channel) -> Box<Future<Item = (), Error = Error>>;
  fn save_channel(&self, channel: &Channel) -> Result<(), Error>;
  fn save_update(&self, update: &UpdateTx) -> Result<(), Error>;
  fn get_counterparty_by_address(&self, &EthAddress) -> Result<Option<Counterparty>, Error>;
  fn get_channel_of_counterparty(&self, &Counterparty) -> Result<Option<Channel>, Error>;
}

pub trait CounterpartyClient {
  fn add_proposed_channel(&self, &str, &NewChannelTx) -> Result<(), Error>;
  fn make_payment(&self, &str, &UpdateTx) -> Box<Future<Item = EthSignature, Error = Error>>;
}

pub trait Crypto {
  fn hash_bytes(&self, &[&[u8]]) -> Bytes32;
  fn eth_sign(&self, &EthPrivateKey, &Bytes32) -> EthSignature;
}

pub trait Blockchain {

}

// pub struct CounterpartyServer {

// }

// impl CounterpartyServer {
//   pub fn make_payment(
//     &self,
//     update_tx: UpdateTx
//   ) -> Result<(), Error> {
//     Ok(())
//   }
// }

pub struct CallerServer<CPT: CounterpartyClient, STO: Storage, CRP: Crypto> {
  pub crypto: CRP,
  pub counterpartyClient: CPT,
  pub storage: STO,
  pub my_eth_address: EthAddress,
  pub challenge_length: Uint256
}

impl<CPT: CounterpartyClient, STO: Storage, CRP: Crypto> CallerServer<CPT, STO, CRP> {
  pub fn open_channel(
    &self,
    amount: Uint256,
    their_eth_address: EthAddress
  ) -> Box<Future<Item = (), Error = Error>> {
    let channel_id = Bytes32([0u8; 32]); // Call eth somehow

    let channel = Channel {
      channel_id,
      address_a: self.my_eth_address,
      address_b: their_eth_address,
      channel_status: ChannelStatus::Open,
      deposit_a: amount,
      deposit_b: 0.into(),
      challenge: self.challenge_length.clone(),
      nonce: 0.into(),
      close_time: 0.into(),
      balance_a: 0.into(),
      balance_b: 0.into(),
      is_a: true
    };

    Box::new(self.storage.new_channel(channel))
  }

  // pub fn join_channel(
  //   &self,
  //   channel_Id: Bytes32,
  //   amount: Uint256
  // ) -> Result<(), Error> {
  //   // Call eth somehow
  //   Ok(())
  // }

  pub fn make_payment(
    self,
    their_address: EthAddress,
    amount: Uint256
  ) -> Box<Future<Item = (), Error = Error>> {
    let counterparty = match self.storage.get_counterparty_by_address(&their_address) {
      Ok(Some(counterparty)) => counterparty,
      Ok(None) => return Box::new(future::err(Error::from(CallerServerError::CounterPartyNotFound {}))),
      Err(err) => return Box::new(future::err(err))
    };
    
    let mut channel = match self.storage.get_channel_of_counterparty(&counterparty) {
      Ok(Some(channel)) => channel,
      Ok(None) => return Box::new(future::err(Error::from(CallerServerError::ChannelNotFound {}))),
      Err(err) => return Box::new(future::err(err))
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

    let fingerprint = self.crypto.hash_bytes(&[
      update_tx.channel_id.as_ref(),
      &update_tx.nonce.to_bytes_le(),
      &update_tx.balance_a.to_bytes_le(),
      &update_tx.balance_b.to_bytes_le()
    ]);

    let my_sig = self.crypto.eth_sign(&EthPrivateKey([0; 64]), &fingerprint);

    update_tx.set_my_signature(channel.is_a, &my_sig);

    self.storage.save_channel(&channel);
    self.storage.save_update(&update_tx);

    Box::new(self.counterpartyClient.make_payment(&counterparty.url, &update_tx)
      .from_err()
      .and_then(|their_signature| {
        update_tx.set_their_signature(channel.is_a, &their_signature);
        match self.storage.save_channel(&channel) {
          Err(err) => return Err(err),
          _ => ()
        };
        match self.storage.save_update(&update_tx) {
          Err(err) => return Err(err),
          _ => ()
        };

        Ok(())
      }))
  }
}

#[cfg(test)]
mod tests {

}
