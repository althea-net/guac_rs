use althea_types::{Bytes32, EthAddress, EthPrivateKey, EthSignature};
use num256::{Int256, Uint256};

#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum ChannelStatus {
  Open,
  Joined,
  Challenge,
  Closed
}

#[derive(Serialize, Deserialize)]
pub struct Channel {
  pub channel_id: Bytes32,
  pub address_a: EthAddress,
  pub address_b: EthAddress,
  pub channel_status: ChannelStatus,
  pub deposit_a: Uint256,
  pub deposit_b: Uint256,
  pub challenge: Uint256,
  pub nonce: Uint256,
  pub close_time: Uint256,
  pub balance_a: Uint256,
  pub balance_b: Uint256,
  pub is_a: bool
}

impl Channel {
  pub fn get_my_address(&self) -> EthAddress {
    match self.is_a {
      true => self.address_a,
      false => self.address_b,
    }
  }
  pub fn get_their_address(&self) -> EthAddress {
    match self.is_a {
      true => self.address_b,
      false => self.address_a,
    }
  }
  pub fn get_my_balance(&self) -> Uint256 {
    match self.is_a.clone() {
      true => self.balance_a.clone(),
      false => self.balance_b.clone(),
    }
  }
  pub fn get_their_balance(&self) -> Uint256 {
    match self.is_a.clone() {
      true => self.balance_b.clone(),
      false => self.balance_a.clone(),
    }
  }
  pub fn set_my_balance(&mut self, balance: &Uint256) {
    match self.is_a {
      true => self.balance_a = balance.clone(),
      false => self.balance_b = balance.clone(),
    }
  }
  pub fn set_their_balance(&mut self, balance: &Uint256) {
    match self.is_a {
      true => self.balance_b = balance.clone(),
      false => self.balance_a = balance.clone(),
    }
  } 
}

#[derive(Serialize, Deserialize)]
pub struct Hashlock {
  pub hash: Bytes32,
  pub amount: Int256,
}

#[derive(Serialize, Deserialize)]
pub struct NewChannelTx {
  pub channel_id: Bytes32,
  pub settling_period: Uint256,
  pub address_a: EthAddress,
  pub address_b: EthAddress,
  pub balance_a: Uint256,
  pub balance_b: Uint256,
  pub signature_a: Option<EthSignature>,
  pub signature_b: Option<EthSignature>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateTx {
  pub channel_id: Bytes32,
  pub nonce: Uint256,

  pub balance_a: Uint256,
  pub balance_b: Uint256,

  pub signature_a: Option<EthSignature>,
  pub signature_b: Option<EthSignature>
}

impl UpdateTx {
  pub fn set_my_signature(&mut self, is_a: bool, signature: &EthSignature) {
    match is_a {
      true => self.signature_a = Some(*signature),
      false => self.signature_b = Some(*signature),
    }
  }
  pub fn set_their_signature(&mut self, is_a: bool, signature: &EthSignature) {
    match is_a {
      true => self.signature_b = Some(*signature),
      false => self.signature_a = Some(*signature),
    }
  }  
}

pub struct Account {
  pub address: EthAddress,
  pub private_key: EthPrivateKey,
  pub balance: Uint256,
}

pub struct Counterparty {
  pub address: EthAddress,
  pub url: String,
}

impl Counterparty {}

pub struct Fullnode {
  pub address: EthAddress,
  pub url: String,
}

#[cfg(test)]
mod tests {
  use serde_json;
  use types;
  #[test]
  fn serialize() {
    // Some data structure.
    let new_channel_tx = types::NewChannelTx {
      address_a: types::Address([7; 20]),
      address_b: types::Address([9; 20]),
      balance_a: 23,
      balance_b: 23,
      channel_id: types::Bytes32([11; 32]),
      settling_period: 45,
      signature_a: None,
      signature_b: None,
    };

    // Serialize it to a JSON string.
    let j = serde_json::to_string(&new_channel_tx).unwrap();

    // Print, write to a file, or send to an HTTP server.
    assert_eq!("{\"channel_id\":\"CwsLCwsLCwsLCwsLCwsLCwsLCwsLCwsLCwsLCwsLCws=\",\"settling_period\":45,\"address_a\":\"BwcHBwcHBwcHBwcHBwcHBwcHBwc=\",\"address_b\":\"CQkJCQkJCQkJCQkJCQkJCQkJCQk=\",\"balance_a\":23,\"balance_b\":23,\"signature_a\":null,\"signature_b\":null}", j);
  }
}
