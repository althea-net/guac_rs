use clarity::{Address, Signature};
use failure::{err_msg, Error};
use num256::Uint256;

use crypto::CryptoService;
use std::ops::Add;
use CRYPTO;

/// This is a state that is able to identity a channel uniquely.
///
/// A channel is registered with New state for a given address, which
/// later identified with a channel id once it arrives in the contract.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub enum ChannelState {
    /// Registered with address of the other party
    New(Address),
    /// Opened with a Channel ID
    Open(Uint256),
    Joined(Uint256),
    Challenge(Uint256),
    Closed(Uint256),
}

impl ChannelState {
    pub fn get_channel_id_ref(&self) -> Option<&Uint256> {
        match *self {
            ChannelState::Open(ref channel_id)
            | ChannelState::Joined(ref channel_id)
            | ChannelState::Challenge(ref channel_id)
            | ChannelState::Closed(ref channel_id) => Some(&channel_id),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Channel {
    pub state: ChannelState,
    pub address_a: Address,
    pub address_b: Address,
    pub deposit_a: Uint256,
    pub deposit_b: Uint256,
    pub challenge: Uint256,
    pub nonce: Uint256,
    pub close_time: Uint256,
    pub balance_a: Uint256,
    pub balance_b: Uint256,
    pub is_a: bool,

    /// URL of the counterparty
    pub url: String,
}

impl Channel {
    pub fn total_deposit(&self) -> Uint256 {
        self.deposit_a.clone() + self.deposit_b.clone()
    }

    pub fn swap(&self) -> Self {
        Channel {
            is_a: !self.is_a,
            ..self.clone()
        }
    }
}

impl Channel {
    pub fn get_my_address(&self) -> &Address {
        match self.is_a {
            true => &self.address_a,
            false => &self.address_b,
        }
    }
    pub fn get_their_address(&self) -> &Address {
        match self.is_a {
            true => &self.address_b,
            false => &self.address_a,
        }
    }
    pub fn my_balance(&self) -> &Uint256 {
        match self.is_a {
            true => &self.balance_a,
            false => &self.balance_b,
        }
    }
    pub fn their_balance(&self) -> &Uint256 {
        match self.is_a {
            true => &self.balance_b,
            false => &self.balance_a,
        }
    }
    pub fn my_balance_mut(&mut self) -> &mut Uint256 {
        match self.is_a {
            true => &mut self.balance_a,
            false => &mut self.balance_b,
        }
    }
    pub fn their_balance_mut(&mut self) -> &mut Uint256 {
        match self.is_a {
            true => &mut self.balance_b,
            false => &mut self.balance_a,
        }
    }
    pub fn my_deposit(&self) -> &Uint256 {
        match self.is_a {
            true => &self.deposit_a,
            false => &self.deposit_b,
        }
    }
    pub fn their_deposit(&self) -> &Uint256 {
        match self.is_a {
            true => &self.deposit_b,
            false => &self.deposit_a,
        }
    }
    pub fn my_deposit_mut(&mut self) -> &mut Uint256 {
        match self.is_a {
            true => &mut self.deposit_a,
            false => &mut self.deposit_b,
        }
    }
    pub fn their_deposit_mut(&mut self) -> &mut Uint256 {
        match self.is_a {
            true => &mut self.deposit_b,
            false => &mut self.deposit_a,
        }
    }
    pub fn create_update(&self) -> Result<UpdateTx, Error> {
        let channel_id = match self.state {
            ChannelState::Open(ref channel_id)
            | ChannelState::Joined(ref channel_id)
            | ChannelState::Challenge(ref channel_id)
            | ChannelState::Closed(ref channel_id) => channel_id.clone(),
            _ => bail!("Unable to create update before channel is open on the network"),
        };

        let mut update_tx = UpdateTx {
            channel_id: channel_id.clone(),
            nonce: self.nonce.clone(),
            balance_a: self.balance_a.clone(),
            balance_b: self.balance_b.clone(),
            signature_a: None,
            signature_b: None,
        };

        update_tx.sign(self.is_a, channel_id.clone());
        Ok(update_tx)
    }
    // pub fn apply_update(&mut self, update: &UpdateTx, validate_balance: bool) -> Result<(), Error> {
    //     trace!(
    //         "Apply update for channel {:?} with {:?}",
    //         self.state,
    //         update.state
    //     );
    //     ensure!(
    //         self.get_channel_id_ref().is_some(),
    //         "Unable to apply update before opening a channel on the network"
    //     );
    //     if update.state != self.state {
    //         bail!("update not for the right channel")
    //     }

    //     if !update.validate_their_signature(self.is_a) {
    //         bail!("sig is bad")
    //     }

    //     ensure!(
    //         update.their_balance(self.is_a).clone() + update.my_balance(self.is_a).clone()
    //             == self.my_balance().clone() + self.their_balance().clone(),
    //         "balance does not add up"
    //     );

    //     ensure!(
    //         update.their_balance(self.is_a).clone() + update.my_balance(self.is_a).clone()
    //             == self.deposit_a.clone() + self.deposit_b.clone(),
    //         "balance does not add up to deposit values"
    //     );

    //     if self.nonce > update.nonce {
    //         bail!("Update too old");
    //     }

    //     if update.my_balance(self.is_a) < self.my_balance() && validate_balance {
    //         bail!("balance validation failed")
    //     }

    //     self.balance_a = update.balance_a.clone();
    //     self.balance_b = update.balance_b.clone();
    //     self.nonce = update.nonce.clone();

    //     Ok(())
    // }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct UpdateTx {
    pub channel_id: Uint256,
    pub nonce: Uint256,

    pub balance_a: Uint256,
    pub balance_b: Uint256,

    pub signature_a: Option<Signature>,
    pub signature_b: Option<Signature>,
}

impl UpdateTx {
    pub fn set_my_signature(&mut self, is_a: bool, signature: &Signature) {
        match is_a {
            true => self.signature_a = Some(signature.clone()),
            false => self.signature_b = Some(signature.clone()),
        }
    }
    pub fn validate_their_signature(&self, _is_a: bool) -> bool {
        // TODO: actually do validation
        true
    }
    pub fn their_balance(&self, is_a: bool) -> &Uint256 {
        match is_a {
            true => &self.balance_b,
            false => &self.balance_a,
        }
    }
    pub fn my_balance(&self, is_a: bool) -> &Uint256 {
        match is_a {
            true => &self.balance_a,
            false => &self.balance_b,
        }
    }
    pub fn set_their_signature(&mut self, is_a: bool, signature: &Signature) {
        match is_a {
            true => self.signature_b = Some(signature.clone()),
            false => self.signature_a = Some(signature.clone()),
        }
    }

    pub fn sign(&mut self, is_a: bool, channel_id: Uint256) {
        let nonce: [u8; 32] = self.nonce.clone().into();
        let balance_a: [u8; 32] = self.balance_a.clone().into();
        let balance_b: [u8; 32] = self.balance_b.clone().into();

        let channel_id: [u8; 32] = channel_id.clone().into();

        let fingerprint = CRYPTO.hash_bytes(&[&channel_id, &nonce, &balance_a, &balance_b]);
        let fingerprint: [u8; 32] = fingerprint.clone().into();

        let my_sig = CRYPTO.eth_sign(&fingerprint);

        self.set_my_signature(is_a, &my_sig.into());
    }

    pub fn strip_sigs(&self) -> UpdateTx {
        UpdateTx {
            signature_a: None,
            signature_b: None,
            ..self.clone()
        }
    }
}
