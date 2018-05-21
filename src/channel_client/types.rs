use althea_types::{Bytes32, EthAddress, EthPrivateKey, EthSignature};
use failure::Error;

use futures::Future;

use ethereum_types::U256;

use counterparty::Counterparty;

use CRYPTO;

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub enum ChannelStatus {
    Open,
    Joined,
    Challenge,
    Closed,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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
    fn new_pair(deposit_a: U256, deposit_b: U256) -> (Channel, Channel) {
        let channel_a = Channel {
            channel_id: Bytes32([0; 32]),
            address_a: EthAddress([0; 20]),
            address_b: EthAddress([0; 20]),
            channel_status: ChannelStatus::Joined,
            deposit_a,
            deposit_b,
            challenge: 0.into(),
            nonce: 0.into(),
            close_time: 10.into(),
            balance_a: deposit_a,
            balance_b: deposit_b,
            is_a: true
        };

        let channel_b = Channel{
            is_a: false,
            ..channel_a
        };

        (channel_a, channel_b)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChannelManager {
    pub state: Channel,
    pub pending_send: U256,
    pub pending_rec: U256,

    pub counterparty: Counterparty,
}

impl ChannelManager {
    fn new_pair(deposit_a: U256, deposit_b: U256) -> (ChannelManager, ChannelManager) {
        let (channel_a, channel_b) = Channel::new_pair(deposit_a, deposit_b);

        let m_a = ChannelManager {
            state: channel_a,
            pending_send: 0.into(),
            pending_rec: 0.into(),
            counterparty: Counterparty {
                address: EthAddress([0; 20]),
                url: String::new(),
            }
        };

        let m_b = ChannelManager {
            state: channel_b,
            ..m_a.clone()
        };

        (m_a, m_b)
    }
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

        if self.pending_send != 0.into() {
            state.nonce += 1.into();

            if state.my_balance() < &self.pending_send {
                bail!("Not enough money in channel")
            }

            *state.my_balance_mut() -= self.pending_send;
            *state.their_balance_mut() += self.pending_send;
        }

        Ok(state.create_update())
    }

    /// This is called by send_payment
    pub fn rec_payment(&mut self, update: UpdateTx) -> Result<(), Error> {
        let (amount_received, amount_sent) = self.state.apply_counterparty_update(&update, self.pending_send)?;
        self.pending_rec += amount_received;
        self.pending_send -= amount_sent;

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

        if rec_update.strip_sigs() != sent_update.strip_sigs() {
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

        if self.nonce > update.nonce {
            bail!("Update too old");
        } else if self.nonce == update.nonce {
            if self.balance_a == update.balance_a && self.balance_b == update.balance_b {
                return Ok(());
            }
        };

        // TODO: Check if validation is good enough

        self.balance_a = update.balance_a;
        self.balance_b = update.balance_b;
        self.nonce = update.nonce;

        Ok(())
    }
    pub fn apply_counterparty_update(&mut self, update: &UpdateTx, pending_send: U256) -> Result<(U256, U256), Error> {
        if update.channel_id != self.channel_id {
            bail!("update not for the right channel")
        }

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

        if self.nonce > update.nonce {
            bail!("Update too old");
        } else if self.nonce == update.nonce {
            if self.balance_a == update.balance_a && self.balance_b == update.balance_b {
                return Ok((0.into(), 0.into()));
            }
        };

        // TODO: Check if validation is good enough

        if update.my_balance(self.is_a) < self.my_balance() {
            let amount_sent = self.my_balance() - update.my_balance(self.is_a);

            if amount_sent <= pending_send {
                self.balance_a = update.balance_a;
                self.balance_b = update.balance_b;
                self.nonce = update.nonce;

                Ok((0.into(), amount_sent))
            }else{
                bail!("cannot accept state update which takes too much money")
            }
        } else {
            let amount_paid = update.my_balance(self.is_a) - self.my_balance();

            self.balance_a = update.balance_a;
            self.balance_b = update.balance_b;
            self.nonce = update.nonce;

            Ok((amount_paid, 0.into()))
        }
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

    pub fn strip_sigs(&self) -> UpdateTx {
        UpdateTx {
            signature_a: None,
            signature_b: None,
            ..self.clone()
        }
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
    fn test_channel_manager_unidirectional_empty() {
        let (mut a, mut b) = ChannelManager::new_pair(100.into(), 100.into());

        let payment = a.create_payment().unwrap();

        b.rec_payment(payment.clone()).unwrap();
        let response = b.create_payment().unwrap();

        a.rec_updated_state(payment, response).unwrap();

        assert_eq!(a.pending_send, 0.into());
        assert_eq!(b.pending_rec, 0.into());
    }

    #[test]
    fn test_channel_manager_unidirectional() {
        let (mut a, mut b) = ChannelManager::new_pair(100.into(), 100.into());

        a.pay_counterparty(20.into());

        let payment = a.create_payment().unwrap();

        b.rec_payment(payment.clone()).unwrap();
        let response = b.create_payment().unwrap();

        a.rec_updated_state(payment, response).unwrap();

        assert_eq!(a.pending_send, 0.into());
        assert_eq!(b.pending_rec, 20.into());
    }

    #[test]
    fn test_channel_manager_bidirectional() {
        let (mut a, mut b) = ChannelManager::new_pair(100.into(), 100.into());

        // A -> B 5
        a.pay_counterparty(5.into()).unwrap();

        let payment = a.create_payment().unwrap();

        b.rec_payment(payment.clone()).unwrap();
        let response = b.create_payment().unwrap();

        a.rec_updated_state(payment, response).unwrap();

        // B -> A 3
        b.pay_counterparty(3.into()).unwrap();

        let payment = b.create_payment().unwrap();

        a.rec_payment(payment.clone()).unwrap();
        let response = a.create_payment().unwrap();

        b.rec_updated_state(payment, response).unwrap();

        assert_eq!(a.pending_send, 0.into());
        assert_eq!(b.pending_send, 0.into());
        assert_eq!(a.pending_rec, 3.into());
        assert_eq!(b.pending_rec, 5.into());
    }

    #[test]
    fn test_channel_manager_bidirectional_race() {
        let (mut a, mut b) = ChannelManager::new_pair(100.into(), 100.into());

        // A -> B 20 and B -> A 20 at the same time
        a.pay_counterparty(3.into()).unwrap();
        b.pay_counterparty(5.into()).unwrap();

        let payment_a = a.create_payment().unwrap();
        let payment_b = b.create_payment().unwrap();

        b.rec_payment(payment_a.clone()).unwrap();
        let response_b = b.create_payment().unwrap();
        a.rec_payment(payment_b.clone()).unwrap();
        let response_a = a.create_payment().unwrap();

        assert!(a.rec_updated_state(payment_a, response_b).is_err()); // these should fail
        assert!(b.rec_updated_state(payment_b, response_a).is_err()); // these should fail

        // unraced request

        let payment = a.create_payment().unwrap();

        b.rec_payment(payment.clone()).unwrap();
        let response = b.create_payment().unwrap();

        a.rec_updated_state(payment, response).unwrap();

        let payment = b.create_payment().unwrap();

        a.rec_payment(payment.clone()).unwrap();
        let response = a.create_payment().unwrap();

        b.rec_updated_state(payment, response).unwrap();


        assert_eq!(a.pending_send, 0.into());
        assert_eq!(b.pending_send, 0.into());
        assert_eq!(a.pending_rec, 5.into());
        assert_eq!(b.pending_rec, 3.into());
    }

    #[test]
    fn test_channel_manager_bidirectional_race_resume() {
        let (mut a, mut b) = ChannelManager::new_pair(100.into(), 100.into());

        // A -> B 20 and B -> A 20 at the same time
        a.pay_counterparty(3.into()).unwrap();
        b.pay_counterparty(5.into()).unwrap();

        let payment_a = a.create_payment().unwrap();
        let payment_b = b.create_payment().unwrap();

        b.rec_payment(payment_a.clone()).unwrap();
        let response_b = b.create_payment().unwrap();
        a.rec_payment(payment_b.clone()).unwrap();
        let response_a = a.create_payment().unwrap();

        assert!(a.rec_updated_state(payment_a, response_b).is_err()); // these should fail
        assert!(b.rec_updated_state(payment_b, response_a).is_err()); // these should fail

        // unraced requests

        // A -> B 1
        a.pay_counterparty(1.into()).unwrap();

        let payment = a.create_payment().unwrap();

        b.rec_payment(payment.clone()).unwrap();
        let response = b.create_payment().unwrap();

        a.rec_updated_state(payment, response).unwrap();

        let payment = b.create_payment().unwrap();

        a.rec_payment(payment.clone()).unwrap();
        let response = a.create_payment().unwrap();

        b.rec_updated_state(payment, response).unwrap();

        assert_eq!(a.pending_send, 0.into());
        assert_eq!(b.pending_send, 0.into());
        assert_eq!(a.pending_rec, 6.into());
        assert_eq!(b.pending_rec, 3.into());
    }
}
