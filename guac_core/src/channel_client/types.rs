use althea_types::{Bytes32, EthAddress, EthSignature};
use failure::Error;

use ethereum_types::U256;

use crypto::CryptoService;
use CRYPTO;
use std::ops::Add;

#[derive(Copy, Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ChannelStatus {
    Open,
    Joined,
    Challenge,
    Closed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Channel {
    pub channel_id: Bytes32,
    pub address_a: EthAddress,
    pub address_b: EthAddress,
    pub channel_status: ChannelStatus,
    pub deposit_a: U256,
    pub deposit_b: U256,
    pub challenge: U256,
    pub nonce: U256,
    pub close_time: U256,
    pub balance_a: U256,
    pub balance_b: U256,
    pub is_a: bool,
}

impl Channel {
    pub fn new_pair(deposit_a: U256, deposit_b: U256) -> (Channel, Channel) {
        let channel_a = Channel {
            channel_id: 0.into(),
            address_a: 1.into(),
            address_b: 2.into(),
            channel_status: ChannelStatus::Joined,
            deposit_a,
            deposit_b,
            challenge: 0.into(),
            nonce: 0.into(),
            close_time: 10.into(),
            balance_a: deposit_a,
            balance_b: deposit_b,
            is_a: true,
        };

        let channel_b = Channel {
            is_a: false,
            ..channel_a
        };

        (channel_a, channel_b)
    }

    pub fn total_deposit(&self) -> U256 {
        self.deposit_a + self.deposit_b
    }

    pub fn swap(&self) -> Self {
        Channel {
            is_a: !self.is_a,
            ..self.clone()
        }
    }
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
    pub fn my_balance(&self) -> &U256 {
        match self.is_a {
            true => &self.balance_a,
            false => &self.balance_b,
        }
    }
    pub fn their_balance(&self) -> &U256 {
        match self.is_a {
            true => &self.balance_b,
            false => &self.balance_a,
        }
    }
    pub fn my_balance_mut(&mut self) -> &mut U256 {
        match self.is_a {
            true => &mut self.balance_a,
            false => &mut self.balance_b,
        }
    }
    pub fn their_balance_mut(&mut self) -> &mut U256 {
        match self.is_a {
            true => &mut self.balance_b,
            false => &mut self.balance_a,
        }
    }
    pub fn my_deposit(&self) -> &U256 {
        match self.is_a {
            true => &self.deposit_a,
            false => &self.deposit_b,
        }
    }
    pub fn their_deposit(&self) -> &U256 {
        match self.is_a {
            true => &self.deposit_b,
            false => &self.deposit_a,
        }
    }
    pub fn my_deposit_mut(&mut self) -> &mut U256 {
        match self.is_a {
            true => &mut self.deposit_a,
            false => &mut self.deposit_b,
        }
    }
    pub fn their_deposit_mut(&mut self) -> &mut U256 {
        match self.is_a {
            true => &mut self.deposit_b,
            false => &mut self.deposit_a,
        }
    }
    pub fn create_update(&self) -> UpdateTx {
        let mut update_tx = UpdateTx {
            channel_id: self.channel_id.clone(),
            nonce: self.nonce.clone(),
            balance_a: self.balance_a.clone(),
            balance_b: self.balance_b.clone(),
            signature_a: None,
            signature_b: None,
        };

        update_tx.sign(self.is_a, self.channel_id.clone());
        update_tx
    }
    pub fn apply_update(&mut self, update: &UpdateTx, validate_balance: bool) -> Result<(), Error> {
        if update.channel_id != self.channel_id {
            bail!("update not for the right channel")
        }

        if !update.validate_their_signature(self.is_a) {
            bail!("sig is bad")
        }

        if update.their_balance(self.is_a).add(update.my_balance(self.is_a).clone())
            != self.my_balance().add(self.their_balance().clone())
        {
            bail!("balance does not add up")
        }

        if update.their_balance(self.is_a).add(update.my_balance(self.is_a).clone())
            != self.deposit_a.add(self.deposit_b.clone())
        {
            bail!("balance does not add up")
        }

        if self.nonce > update.nonce {
            bail!("Update too old");
        }

        if update.my_balance(self.is_a) < self.my_balance() && validate_balance {
            bail!("balance validation failed")
        }

        self.balance_a = update.balance_a;
        self.balance_b = update.balance_b;
        self.nonce = update.nonce;

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct NewChannelTx {
    pub to: EthAddress,
    pub challenge: U256,
    pub deposit: U256,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct UpdateTx {
    pub channel_id: Bytes32,
    pub nonce: U256,

    pub balance_a: U256,
    pub balance_b: U256,

    pub signature_a: Option<EthSignature>,
    pub signature_b: Option<EthSignature>,
}

impl UpdateTx {
    pub fn set_my_signature(&mut self, is_a: bool, signature: &EthSignature) {
        match is_a {
            true => self.signature_a = Some(*signature),
            false => self.signature_b = Some(*signature),
        }
    }
    pub fn validate_their_signature(&self, _is_a: bool) -> bool {
        // TODO: actually do validation
        true
    }
    pub fn their_balance(&self, is_a: bool) -> &U256 {
        match is_a {
            true => &self.balance_b,
            false => &self.balance_a,
        }
    }
    pub fn my_balance(&self, is_a: bool) -> &U256 {
        match is_a {
            true => &self.balance_a,
            false => &self.balance_b,
        }
    }
    pub fn set_their_signature(&mut self, is_a: bool, signature: &EthSignature) {
        match is_a {
            true => self.signature_b = Some(*signature),
            false => self.signature_a = Some(*signature),
        }
    }

    pub fn sign(&mut self, is_a: bool, channel_id: Bytes32) {
        let mut nonce = [0u8; 32];
        self.nonce.to_big_endian(&mut nonce);
        let mut balance_a = [0u8; 32];
        self.balance_a.to_big_endian(&mut balance_a);
        let mut balance_b = [0u8; 32];
        self.balance_b.to_big_endian(&mut balance_b);

        let channel_id: [u8; 32] = channel_id.into();

        let fingerprint = CRYPTO.hash_bytes(&[&channel_id, &nonce, &balance_a, &balance_b]);
        let fingerprint: [u8; 32] = fingerprint.into();

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
