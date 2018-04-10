extern crate rand;

use althea_types::{EthAddress};
use crypto::Crypto;
use failure::Error;
use num256::Uint256;
use types::{Channel, Counterparty, UpdateTx, NewChannelTx};

#[derive(Debug, Fail)]
enum LogicError {
  #[fail(display = "Could not find counterparty")]
  CounterPartyNotFound {},
  #[fail(display = "Could not find channel")]
  ChannelNotFound {},
}

pub trait Storage {
  fn new_channel(&self, channel: Channel) -> Result<(), Error>;
  fn get_counterparty_by_address(&self, eth_addr: &EthAddress) -> Result<Option<Counterparty>, Error>;
  fn get_channel_of_counterparty(&self, counterparty: &Counterparty) -> Result<Option<Channel>, Error>;
}

struct DB {}
impl Storage for DB {
  fn new_channel(&self, channel: Channel) -> Result<(), Error> {
    Ok(())
  }
  fn get_counterparty_by_address(&self, eth_addr: &EthAddress) -> Result<Option<Counterparty>, Error> {
    Ok(None)
  }
  fn get_channel_of_counterparty(&self, counterparty: &Counterparty) -> Result<Option<Channel>, Error> {
    Ok(None)
  }
}

pub trait CounterpartyAPI {
  fn add_proposed_channel(&self, url: &str, nc: NewChannelTx) -> Result<(), Error>;
  fn make_payment(&self, &str, UpdateTx) -> Result<(), Error>;
}

struct Network {}
impl CounterpartyAPI for Network {
  fn add_proposed_channel(&self, url: &str, nc: NewChannelTx) -> Result<(), Error> {
    Ok(())
  }
  fn make_payment(&self, url: &str, update_tx: UpdateTx) -> Result<(), Error> {
    Ok(())
  }
}

pub struct Logic<CP: CounterpartyAPI, ST: Storage> {
  pub crypto: Crypto,
  pub counterpartyAPI: CP,
  pub storage: ST,
}

impl<CP: CounterpartyAPI, ST: Storage> Logic<CP, ST> {
  // pub fn propose_channel(
  //   self,
  //   channel_id: Bytes32,
  //   my_address: EthAddress,
  //   their_address: EthAddress,
  //   my_balance: Uint256,
  //   their_balance: Uint256,
  //   settling_period: Uint256,
  // ) -> Result<(), Error> {
  //   let channel = Channel::new(
  //     channel_id,
  //     my_address,
  //     their_address,
  //     my_balance.clone(),
  //     their_balance.clone(),
  //     Participant::Zero,
  //   );

  //   try!(self.storage.new_channel(channel));

  //   let mut tx = NewChannelTx {
  //     channel_id: Bytes32([0; 32]),
  //     address0: my_address,
  //     address1: their_address,
  //     balance0: my_balance,
  //     balance1: their_balance,
  //     settling_period,
  //     signature0: None,
  //     signature1: None,
  //   };

  //   tx.signature0 = Some(try!(self.crypto.sign(&my_address, &tx.get_fingerprint())));

  //   let counterparty = match try!(self.storage.get_counterparty(&their_address)) {
  //     Some(counterparty) => counterparty,
  //     None => return Err(Error::from(LogicError::CounterPartyNotFound {})),
  //   };

  //   try!(
  //     self
  //       .counterpartyAPI
  //       .add_proposed_channel(&counterparty.url, tx)
  //   );

  //   Ok(())
  // }

// pub struct UpdateTx {
//   pub channelId: Bytes32,
//   pub sequenceNumber: Uint256,

//   pub balance0: Uint256,
//   pub balance1: Uint256,

//   pub hashlocks: Vec<Hashlock>,

//   pub signature0: EthSignature,
//   pub signature1: EthSignature
// }

  pub fn makePayment(
    self,
    their_address: EthAddress,
    amount: Uint256
  ) -> Result<(), Error> {
    let counterparty = match self.storage.get_counterparty_by_address(&their_address)? {
      Some(counterparty) => counterparty,
      None => return Err(Error::from(LogicError::CounterPartyNotFound {})), 
    };
    let channel = match self.storage.get_channel_of_counterparty(&counterparty)? {
      Some(channel) => channel,
      None => return Err(Error::from(LogicError::ChannelNotFound {})), 
    };

    channel.sequence_number = channel.sequence_number + 1;
    channel.set_my_balance(channel.get_my_balance() - amount);
    channel.set_their_balance(channel.get_their_balance() + amount);

    let update_tx = UpdateTx {
      channel_id: channel.channel_id,
      sequence_number: channel.sequence_number + 1,
      balance0: channel.balance0,
      balance1: channel.balance1,
      hashlocks: channel.hashlocks,
      signature0: None,
      signature1: None,
    };

    update_tx.sign();

    self.storage.save_channel(channel);
    self.storage.save_update(update_tx);

    let their_signature = 
      self.counterpartyAPI.make_payment(counterparty.url, channel.update_tx)?;

    channel.set_their_signature(their_signature);
    update_tx.set_their_signature(their_signature);

    self.storage.save_channel(channel);
    self.storage.save_update(update_tx);
  }
}

#[cfg(test)]
mod tests {

}
