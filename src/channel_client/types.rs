use althea_types::{Bytes32, EthAddress, EthPrivateKey, EthSignature};
use num256::{Int256, Uint256};
use failure::{Error};

use CRYPTO;

#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum ChannelStatus {
    Open,
    Joined,
    Challenge,
    Closed,
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
    pub is_a: bool,
}

pub struct ChannelManager {
    their_state: Channel,
    their_sig: Bytes32,

    my_state: Channel,
}

impl ChannelManager {
    fn pay_counterparty(&mut self, amount: Uint256) -> Result<ChannelUpdate, Error> {
        self.my_state.nonce += 1;
        *self.my_state.their_balance_mut() -= amount.clone();
        *self.my_state.my_balance_mut() += amount.clone();

        let payment_amount: Uint256 = self.my_state.their_balance() - self.their_state.their_balance();

        Ok(ChannelUpdate{
            tx: self.my_state.create_update(),
            base_nonce: self.their_state.nonce.clone(),
            payment: payment_amount
        })
    }

    fn payment_recieved(&mut self, update: ChannelUpdate) -> Result<(), Error> {
        if update.base_nonce != self.my_state.nonce {
            bail!("Payment in flight already")
        }

        // TODO: Check if this validation is ok

        self.my_state.apply_counterparty_update(update.tx.clone())?;
        self.their_state.apply_counterparty_update(update.tx.clone())?;
        Ok(())
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
    pub fn my_balance(&self) -> &Uint256 {
        match self.is_a.clone() {
            true => &self.balance_a,
            false => &self.balance_b,
        }
    }
    pub fn their_balance(&self) -> &Uint256 {
        match self.is_a.clone() {
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
    pub fn apply_counterparty_update(&mut self, update: UpdateTx) -> Result<(), Error> {
        if self.nonce >= update.nonce {
            bail!("Update too old");
        };

        if !update.val_their_signature(self.is_a) {
            bail!("sig is bad")
        }

        if update.their_balance(self.is_a) + update.my_balance(self.is_a) != self.my_balance() + self.their_balance() {
            bail!("balance does not add up")
        }

        if update.their_balance(self.is_a) + update.my_balance(self.is_a) != self.deposit_a.clone() + self.deposit_b.clone() {
            bail!("balance does not add up")
        }

        if update.my_balance(self.is_a) < self.my_balance() {
            bail!("payments can only give money")
        }

        if update.channel_id != self.channel_id {
            bail!("update not for the right channel")
        }

        // TODO: Check if validation is good enough

        self.balance_a = update.balance_a;
        self.balance_b = update.balance_b;
        self.nonce = update.nonce;

        Ok(())
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateTx {
    pub channel_id: Bytes32,
    pub nonce: Uint256,

    pub balance_a: Uint256,
    pub balance_b: Uint256,

    pub signature_a: Option<EthSignature>,
    pub signature_b: Option<EthSignature>,
}

pub struct ChannelUpdate {
    tx: UpdateTx,

    /// the last nonce which I have your signature for
    base_nonce: Uint256,

    /// the payments made since the base_nonce
    payment: Uint256,
}

impl UpdateTx {
    pub fn set_my_signature(&mut self, is_a: bool, signature: &EthSignature) {
        match is_a {
            true => self.signature_a = Some(*signature),
            false => self.signature_b = Some(*signature),
        }
    }
    pub fn val_their_signature(&self, is_a: bool) -> bool {
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
    pub fn set_their_signature(&mut self, is_a: bool, signature: &EthSignature) {
        match is_a {
            true => self.signature_b = Some(*signature),
            false => self.signature_a = Some(*signature),
        }
    }

    pub fn sign(&mut self, is_a: bool, channel_id: Bytes32) {
        let fingerprint = CRYPTO.hash_bytes(&[
            channel_id.as_ref(),
            &self.nonce.to_bytes_le(),
            &self.balance_a.to_bytes_le(),
            &self.balance_b.to_bytes_le(),
        ]);

        let my_sig = CRYPTO.eth_sign(&fingerprint);

        self.set_my_signature(is_a, &my_sig);
    }
}

#[cfg(test)]
mod tests {
    use serde_json;
    use super::*;
    #[test]
    fn serialize() {
        // Some data structure.
        let new_channel_tx = NewChannelTx {
            address_a: EthAddress([7; 20]),
            address_b: EthAddress([9; 20]),
            balance_a: 23.into(),
            balance_b: 23.into(),
            channel_id: Bytes32([11; 32]),
            settling_period: 45.into(),
            signature_a: None,
            signature_b: None,
        };

        // Serialize it to a JSON string.
        let j = serde_json::to_string(&new_channel_tx).unwrap();

        // Print, write to a file, or send to an HTTP server.
        assert_eq!("{\"channel_id\":\"0x0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b\",\"settling_period\":\"45\",\"address_a\":\"0x0707070707070707070707070707070707070707\",\"address_b\":\"0x0909090909090909090909090909090909090909\",\"balance_a\":\"23\",\"balance_b\":\"23\",\"signature_a\":null,\"signature_b\":null}", j);
    }
}
