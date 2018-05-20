use althea_types::{Bytes32, EthAddress, EthPrivateKey, EthSignature};
use failure::Error;

use futures::Future;

use ethereum_types::U256;

use counterparty::Counterparty;

use CRYPTO;

#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum ChannelStatus {
    Open,
    Joined,
    Challenge,
    Closed,
}

#[derive(Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Deserialize, Clone)]
pub struct ChannelManager {
    pub state: Channel,
    pub pending_send: U256,
    pub pending_rec: U256,

    pub counterparty: Counterparty,
}

impl ChannelManager {
    /// Function to pay counterparty, doesn't actually send anything
    pub fn pay_counterparty(&mut self, amount: U256) -> Result<(), Error> {
        self.pending_send += amount;
        Ok(())
    }

    /// This sums up the pending amount and returns a channel update
    pub fn create_payment(&mut self) -> Result<UpdateTx, Error> {
        let mut state = self.state.clone();

        state.nonce += 1.into();

        if state.my_balance() < &self.pending_send {
            bail!("Not enough money in channel")
        }

        *state.my_balance_mut() -= self.pending_send;
        *state.their_balance_mut() += self.pending_send;

        Ok(state.create_update())
    }

    /// This is called by send_payment
    pub fn rec_payment(&mut self, update: UpdateTx) -> Result<(), Error> {
        let old_balance = self.state.my_balance().clone();
        self.state.apply_counterparty_update(&update)?;
        let payment_amt = self.state.my_balance() - old_balance;

        self.pending_rec += payment_amt;

        Ok(())
    }

    /// This is called on the response to rec_payment
    pub fn rec_updated_state(
        &mut self,
        sent_update: UpdateTx,
        rec_update: UpdateTx,
    ) -> Result<(), Error> {
        let mut self_ = self.clone(); // this update must be atomic

        let their_old_balance = self_.state.their_balance().clone();
        self_.state.apply_own_update(&sent_update)?;
        let payment_amt_to_them = self_.state.their_balance() - their_old_balance;

        if payment_amt_to_them > self_.pending_send {
            bail!("somehow sent too much");
        }

        self_.pending_send -= payment_amt_to_them;

        if rec_update != sent_update {
            self_.rec_payment(rec_update)?;
        };

        *self = self_;

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
    pub fn my_balance(&self) -> &U256 {
        match self.is_a.clone() {
            true => &self.balance_a,
            false => &self.balance_b,
        }
    }
    pub fn their_balance(&self) -> &U256 {
        match self.is_a.clone() {
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
    pub fn apply_own_update(&mut self, update: &UpdateTx) -> Result<(), Error> {
        if self.nonce >= update.nonce {
            bail!("Update too old");
        }

        if update.their_balance(self.is_a) + update.my_balance(self.is_a)
            != self.my_balance() + self.their_balance()
        {
            bail!("balance does not add up")
        }

        if update.their_balance(self.is_a) + update.my_balance(self.is_a)
            != self.deposit_a.clone() + self.deposit_b.clone()
        {
            bail!("balance does not add up")
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
    pub fn apply_counterparty_update(&mut self, update: &UpdateTx) -> Result<(), Error> {
        if self.nonce >= update.nonce {
            bail!("Update too old");
        };

        if !update.val_their_signature(self.is_a) {
            bail!("sig is bad")
        }

        if update.their_balance(self.is_a) + update.my_balance(self.is_a)
            != self.my_balance() + self.their_balance()
        {
            bail!("balance does not add up")
        }

        if update.their_balance(self.is_a) + update.my_balance(self.is_a)
            != self.deposit_a.clone() + self.deposit_b.clone()
        {
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
    pub fn val_their_signature(&self, is_a: bool) -> bool {
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

        let fingerprint = CRYPTO.hash_bytes(&[channel_id.as_ref(), &nonce, &balance_a, &balance_b]);

        let my_sig = CRYPTO.eth_sign(&fingerprint.0);

        self.set_my_signature(is_a, &my_sig);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    #[test]
    fn serialize() {
        // Some data structure.
        let update_tx = UpdateTx {
            balance_a: 23.into(),
            balance_b: 23.into(),
            channel_id: Bytes32([11; 32]),
            nonce: 45.into(),
            signature_a: None,
            signature_b: None,
        };

        // Serialize it to a JSON string.
        let j = serde_json::to_string(&update_tx).unwrap();

        // Print, write to a file, or send to an HTTP server.
        assert_eq!("{\"channel_id\":\"0x0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b\",\"nonce\":\"0x2d\",\"balance_a\":\"0x17\",\"balance_b\":\"0x17\",\"signature_a\":null,\"signature_b\":null}", j);
    }

    #[test]
    fn test_channel_manager_send_happy() {}
}
