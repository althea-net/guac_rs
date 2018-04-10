use althea_types::{Bytes32, EthAddress, EthPrivateKey, EthSignature};
use num256::{Int256, Uint256};
use tiny_keccak::Keccak;

#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum Participant {
  Zero = 0,
  One = 1,
}

#[derive(Serialize, Deserialize)]
pub struct Channel {
  pub channel_id: Bytes32,
  pub address0: EthAddress,
  pub address1: EthAddress,
  pub ended: bool,
  pub closed: bool,
  pub balance0: Uint256,
  pub balance1: Uint256,
  pub total_balance: Uint256,
  pub hashlocks: Vec<Hashlock>,
  pub sequence_number: Uint256,
  pub participant: Participant,
}

impl Channel {
  pub fn new(
    channel_id: Bytes32,
    address0: EthAddress,
    address1: EthAddress,
    balance0: Uint256,
    balance1: Uint256,
    participant: Participant,
  ) -> Channel {
    Channel {
      channel_id,
      address0,
      address1,
      balance0: balance0.clone(),
      balance1: balance1.clone(),
      participant,
      total_balance: balance0 + balance1,

      sequence_number: Uint256::from(0 as u32),
      closed: false,
      ended: false,
      hashlocks: Vec::new(),
    }
  }

  pub fn get_my_address(&self) -> EthAddress {
    match self.participant {
      Participant::Zero => self.address0,
      Participant::One => self.address1,
    }
  }
  pub fn get_their_address(&self) -> EthAddress {
    match self.participant {
      Participant::Zero => self.address1,
      Participant::One => self.address0,
    }
  }
  pub fn get_my_balance(&self) -> Uint256 {
    match self.participant {
      Participant::Zero => self.balance0.clone(),
      Participant::One => self.balance1.clone(),
    }
  }
  pub fn get_their_balance(&self) -> Uint256 {
    match self.participant {
      Participant::Zero => self.balance1.clone(),
      Participant::One => self.balance0.clone(),
    }
  }
  pub fn set_my_balance(&self, balance: Uint256) {
    match self.participant {
      Participant::Zero => self.balance0 = balance,
      Participant::One => self.balance1 = balance,
    }
  }
  pub fn set_their_balance(&self, balance: Uint256) {
    match self.participant {
      Participant::Zero => self.balance1 = balance,
      Participant::One => self.balance0 = balance,
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
  pub address0: EthAddress,
  pub address1: EthAddress,
  pub balance0: Uint256,
  pub balance1: Uint256,
  pub signature0: Option<EthSignature>,
  pub signature1: Option<EthSignature>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateTx {
  pub channel_id: Bytes32,
  pub sequence_number: Uint256,

  pub balance0: Uint256,
  pub balance1: Uint256,

  pub hashlocks: Vec<Hashlock>,

  pub signature0: Option<EthSignature>,
  pub signature1: Option<EthSignature>
}

impl UpdateTx {
  pub fn get_fingerprint(&self) -> Bytes32 {
    let mut keccak = Keccak::new_keccak256();
    let mut result = [0u8; 32];
    keccak.update(self.channel_id.as_ref());
    keccak.update(&self.sequence_number.to_bytes_le());
    keccak.update(&self.balance0.to_bytes_le());
    keccak.update(&self.balance1.to_bytes_le());

    for hashlock in self.hashlocks {
      keccak.update(hashlock.as_ref());
    }

    keccak.finalize(&mut result);
    Bytes32(result)
  }
}

impl NewChannelTx {
  pub fn get_fingerprint(&self) -> Bytes32 {
    let mut keccak = Keccak::new_keccak256();
    let mut result = [0u8; 32];
    keccak.update(self.channel_id.as_ref());
    keccak.update(&self.settling_period.to_bytes_le());
    keccak.update(self.address0.as_ref());
    keccak.update(self.address1.as_ref());
    keccak.update(&self.balance0.to_bytes_le());
    keccak.update(&self.balance1.to_bytes_le());
    keccak.finalize(&mut result);
    Bytes32(result)
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
      address0: types::Address([7; 20]),
      address1: types::Address([9; 20]),
      balance0: 23,
      balance1: 23,
      channel_id: types::Bytes32([11; 32]),
      settling_period: 45,
      signature0: None,
      signature1: None,
    };

    // Serialize it to a JSON string.
    let j = serde_json::to_string(&new_channel_tx).unwrap();

    // Print, write to a file, or send to an HTTP server.
    assert_eq!("{\"channel_id\":\"CwsLCwsLCwsLCwsLCwsLCwsLCwsLCwsLCwsLCwsLCws=\",\"settling_period\":45,\"address0\":\"BwcHBwcHBwcHBwcHBwcHBwcHBwc=\",\"address1\":\"CQkJCQkJCQkJCQkJCQkJCQkJCQk=\",\"balance0\":23,\"balance1\":23,\"signature0\":null,\"signature1\":null}", j);
  }
}
