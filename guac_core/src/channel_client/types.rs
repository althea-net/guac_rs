use clarity::{Address, Signature};
use failure::{err_msg, Error};
use num256::Uint256;

use crypto::CryptoService;
use CRYPTO;

#[derive(Copy, Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ChannelStatus {
    Open,
    Joined,
    Challenge,
    Closed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Channel {
    pub channel_id: Option<Uint256>,
    pub address_a: Address,
    pub address_b: Address,
    pub channel_status: ChannelStatus,
    pub deposit_a: Uint256,
    pub deposit_b: Uint256,
    pub challenge: Uint256,
    pub nonce: Uint256,
    pub close_time: Uint256,
    pub balance_a: Uint256,
    pub balance_b: Uint256,
    pub is_a: bool,
}

impl Channel {
    pub fn new_pair(
        channel_id: &Uint256,
        deposit_a: Uint256,
        deposit_b: Uint256,
    ) -> (Channel, Channel) {
        let channel_a = Channel {
            channel_id: Some(channel_id.clone()),
            address_a: "0x0000000000000000000000000000000000000001"
                .parse()
                .unwrap(),
            address_b: "0x0000000000000000000000000000000000000002"
                .parse()
                .unwrap(),
            channel_status: ChannelStatus::Joined,
            deposit_a: deposit_a.clone(),
            deposit_b: deposit_b.clone(),
            challenge: 0u64.into(),
            nonce: 0u64.into(),
            close_time: 10u64.into(),
            balance_a: deposit_a,
            balance_b: deposit_b,
            is_a: true,
        };

        let channel_b = Channel {
            is_a: false,
            ..channel_a.clone()
        };

        (channel_a, channel_b)
    }

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
        if self.is_a {
            &self.address_a
        } else {
            &self.address_b
        }
    }
    pub fn get_their_address(&self) -> &Address {
        if self.is_a {
            &self.address_b
        } else {
            &self.address_a
        }
    }
    pub fn my_balance(&self) -> &Uint256 {
        if self.is_a {
            &self.balance_a
        } else {
            &self.balance_b
        }
    }
    pub fn their_balance(&self) -> &Uint256 {
        if self.is_a {
            &self.balance_b
        } else {
            &self.balance_a
        }
    }
    pub fn my_balance_mut(&mut self) -> &mut Uint256 {
        if self.is_a {
            &mut self.balance_a
        } else {
            &mut self.balance_b
        }
    }
    pub fn their_balance_mut(&mut self) -> &mut Uint256 {
        if self.is_a {
            &mut self.balance_b
        } else {
            &mut self.balance_a
        }
    }
    pub fn my_deposit(&self) -> &Uint256 {
        if self.is_a {
            &self.deposit_a
        } else {
            &self.deposit_b
        }
    }
    pub fn their_deposit(&self) -> &Uint256 {
        if self.is_a {
            &self.deposit_b
        } else {
            &self.deposit_a
        }
    }
    pub fn my_deposit_mut(&mut self) -> &mut Uint256 {
        if self.is_a {
            &mut self.deposit_a
        } else {
            &mut self.deposit_b
        }
    }
    pub fn their_deposit_mut(&mut self) -> &mut Uint256 {
        if self.is_a {
            &mut self.deposit_b
        } else {
            &mut self.deposit_a
        }
    }
    pub fn create_update(&self) -> Result<UpdateTx, Error> {
        let channel_id = self.channel_id.as_ref().ok_or_else(|| {
            err_msg("Unable to create update before channel is open on the network")
        })?;

        let mut update_tx = UpdateTx {
            channel_id: channel_id.clone(),
            nonce: self.nonce.clone(),
            balance_a: self.balance_a.clone(),
            balance_b: self.balance_b.clone(),
            signature_a: None,
            signature_b: None,
        };

        update_tx.sign(self.is_a, &channel_id);
        Ok(update_tx)
    }
    pub fn apply_update(&mut self, update: &UpdateTx, validate_balance: bool) -> Result<(), Error> {
        trace!(
            "Apply update for channel {:?} with {:?}",
            self.channel_id,
            update.channel_id
        );
        ensure!(
            self.channel_id.is_some(),
            "Unable to apply update before opening a channel on the network"
        );
        if update.channel_id != *self.channel_id.as_ref().unwrap() {
            bail!("update not for the right channel")
        }

        if !update.validate_their_signature(self.is_a) {
            bail!("sig is bad")
        }

        ensure!(
            update.their_balance(self.is_a).clone() + update.my_balance(self.is_a).clone()
                == self.my_balance().clone() + self.their_balance().clone(),
            "balance does not add up"
        );

        ensure!(
            update.their_balance(self.is_a).clone() + update.my_balance(self.is_a).clone()
                == self.deposit_a.clone() + self.deposit_b.clone(),
            "balance does not add up to deposit values"
        );

        if self.nonce > update.nonce {
            bail!("Update too old");
        }

        if update.my_balance(self.is_a) < self.my_balance() && validate_balance {
            bail!("balance validation failed")
        }

        self.balance_a = update.balance_a.clone();
        self.balance_b = update.balance_b.clone();
        self.nonce = update.nonce.clone();

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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
        if is_a {
            self.signature_a = Some(signature.clone());
        } else {
            self.signature_b = Some(signature.clone());
        }
    }
    pub fn validate_their_signature(&self, _is_a: bool) -> bool {
        // TODO: actually do validation
        true
    }
    pub fn their_balance(&self, is_a: bool) -> &Uint256 {
        if is_a {
            &self.balance_b
        } else {
            &self.balance_a
        }
    }
    pub fn my_balance(&self, is_a: bool) -> &Uint256 {
        if is_a {
            &self.balance_a
        } else {
            &self.balance_b
        }
    }
    pub fn set_their_signature(&mut self, is_a: bool, signature: &Signature) {
        if is_a {
            self.signature_b = Some(signature.clone())
        } else {
            self.signature_a = Some(signature.clone())
        }
    }

    pub fn sign(&mut self, is_a: bool, channel_id: &Uint256) {
        let nonce: [u8; 32] = self.nonce.clone().into();
        let balance_a: [u8; 32] = self.balance_a.clone().into();
        let balance_b: [u8; 32] = self.balance_b.clone().into();

        let channel_id: [u8; 32] = channel_id.clone().into();

        let fingerprint = CRYPTO.hash_bytes(&[&channel_id, &nonce, &balance_a, &balance_b]);
        let fingerprint: [u8; 32] = fingerprint.clone().into();

        let my_sig = CRYPTO.eth_sign(&fingerprint);

        self.set_my_signature(is_a, &my_sig);
    }

    pub fn strip_sigs(&self) -> UpdateTx {
        UpdateTx {
            signature_a: None,
            signature_b: None,
            ..self.clone()
        }
    }
}
