use clarity::{Address, BigEndianInt, Signature};
use failure::Error;

use crypto::CryptoService;
use std::ops::Add;
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
    pub channel_id: BigEndianInt,
    pub address_a: Address,
    pub address_b: Address,
    pub channel_status: ChannelStatus,
    pub deposit_a: BigEndianInt,
    pub deposit_b: BigEndianInt,
    pub challenge: BigEndianInt,
    pub nonce: BigEndianInt,
    pub close_time: BigEndianInt,
    pub balance_a: BigEndianInt,
    pub balance_b: BigEndianInt,
    pub is_a: bool,
}

impl Channel {
    pub fn new_pair(deposit_a: BigEndianInt, deposit_b: BigEndianInt) -> (Channel, Channel) {
        let channel_a = Channel {
            channel_id: 0u64.into(),
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

    pub fn total_deposit(&self) -> BigEndianInt {
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
    pub fn my_balance(&self) -> &BigEndianInt {
        match self.is_a {
            true => &self.balance_a,
            false => &self.balance_b,
        }
    }
    pub fn their_balance(&self) -> &BigEndianInt {
        match self.is_a {
            true => &self.balance_b,
            false => &self.balance_a,
        }
    }
    pub fn my_balance_mut(&mut self) -> &mut BigEndianInt {
        match self.is_a {
            true => &mut self.balance_a,
            false => &mut self.balance_b,
        }
    }
    pub fn their_balance_mut(&mut self) -> &mut BigEndianInt {
        match self.is_a {
            true => &mut self.balance_b,
            false => &mut self.balance_a,
        }
    }
    pub fn my_deposit(&self) -> &BigEndianInt {
        match self.is_a {
            true => &self.deposit_a,
            false => &self.deposit_b,
        }
    }
    pub fn their_deposit(&self) -> &BigEndianInt {
        match self.is_a {
            true => &self.deposit_b,
            false => &self.deposit_a,
        }
    }
    pub fn my_deposit_mut(&mut self) -> &mut BigEndianInt {
        match self.is_a {
            true => &mut self.deposit_a,
            false => &mut self.deposit_b,
        }
    }
    pub fn their_deposit_mut(&mut self) -> &mut BigEndianInt {
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

        if update
            .their_balance(self.is_a)
            .clone()
            .add(update.my_balance(self.is_a).clone())
            != self.my_balance().clone().add(self.their_balance().clone())
        {
            bail!("balance does not add up")
        }

        if update
            .their_balance(self.is_a)
            .clone()
            .add(update.my_balance(self.is_a).clone())
            != self.deposit_a.clone().add(self.deposit_b.clone())
        {
            bail!("balance does not add up")
        }

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

#[derive(Serialize)]
pub struct NewChannelTx {
    pub to: Address,
    pub challenge: BigEndianInt,
    pub deposit: BigEndianInt,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UpdateTx {
    pub channel_id: BigEndianInt,
    pub nonce: BigEndianInt,

    pub balance_a: BigEndianInt,
    pub balance_b: BigEndianInt,

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
    pub fn their_balance(&self, is_a: bool) -> &BigEndianInt {
        match is_a {
            true => &self.balance_b,
            false => &self.balance_a,
        }
    }
    pub fn my_balance(&self, is_a: bool) -> &BigEndianInt {
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

    pub fn sign(&mut self, is_a: bool, channel_id: BigEndianInt) {
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
