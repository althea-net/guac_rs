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

    fn total_deposit(&self) -> U256 {
        self.deposit_a + self.deposit_b
    }

    fn swap(&self) -> Self {
        Channel {
            is_a: !self.is_a,
            ..self.clone()
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ChannelManager {
    New,
    Proposed {
        state: Channel,
        accepted: bool,
    },
    PendingOpening {
        state: Channel,
    },
    PendingJoin {
        state: Channel,
    },
    Open {
        their_state: Channel,
        my_state: Channel,

        pending_rec: U256,
    }, // TODO: close/dispute
}

/// If we should accept their proposal
fn is_proposal_acceptable(state: &Channel) -> Result<bool, Error> {
    Ok(true)
}

impl ChannelManager {
    fn new() -> ChannelManager {
        ChannelManager::New
    }

    fn propose_channel(&mut self) -> Result<Channel, Error> {
        match self {
            ChannelManager::New => {
                // TODO make the defaults configurable
                let proposal = Channel {
                    channel_id: 0.into(),
                    address_a: 1.into(),
                    address_b: 2.into(),
                    channel_status: ChannelStatus::Joined,
                    deposit_a: 1000.into(),
                    deposit_b: 0.into(),
                    challenge: 0.into(),
                    nonce: 0.into(),
                    close_time: 10.into(),
                    balance_a: 1000.into(),
                    balance_b: 0.into(),
                    is_a: true,
                };
                *self = ChannelManager::Proposed {
                    accepted: false,
                    state: proposal.clone(),
                };
                Ok(proposal)
            }
            ChannelManager::Proposed {
                accepted: false,
                state,
            } => Ok(state.clone()),
            _ => bail!("can only propose if not accepted and in state Proposed"),
        }
    }

    fn check_proposal(&mut self, their_prop: Channel) -> Result<bool, Error> {
        if is_proposal_acceptable(&their_prop.swap())? {
            *self = match self {
                ChannelManager::Proposed {
                    accepted: false,
                    state,
                } => {
                    let our_prop = state;
                    assert!(their_prop.address_a != our_prop.address_a);
                    // smallest address wins
                    if their_prop.address_a > our_prop.address_a {
                        bail!("our address is lower, rejecting")
                    } else {
                        // use their proposal
                        ChannelManager::Proposed {
                            accepted: true,
                            state: their_prop.swap(),
                        }
                    }
                }
                // accept if new
                ChannelManager::New => ChannelManager::PendingOpening {
                    state: their_prop.swap().clone(),
                },
                _ => bail!("cannot accept proposal if not in New or Proposed"),
            }
        } else {
            bail!("Cannot accept proposal")
        }

        Ok(true)
    }

    fn new_open_pair(deposit_a: U256, deposit_b: U256) -> (ChannelManager, ChannelManager) {
        let (channel_a, channel_b) = Channel::new_pair(deposit_a, deposit_b);

        let m_a = ChannelManager::Open {
            their_state: channel_a.clone(),
            my_state: channel_a.clone(),
            pending_rec: 0.into(),
        };

        let m_b = ChannelManager::Open {
            their_state: channel_b.clone(),
            my_state: channel_b.clone(),
            pending_rec: 0.into(),
        };

        (m_a, m_b)
    }
}

impl ChannelManager {
    /// Function to pay counterparty, doesn't actually send anything
    pub fn pay_counterparty(&mut self, amount: U256) -> Result<(), Error> {
        match self {
            ChannelManager::Open {
                my_state,
                their_state,
                pending_rec,
            } => {
                *my_state.my_balance_mut() -= amount;
                *my_state.their_balance_mut() += amount;
                Ok(())
            }
            // TODO: Handle close and dispute
            _ => bail!("can only pay in open state"),
        }
    }

    pub fn withdraw(&mut self) -> Result<U256, Error> {
        match self {
            ChannelManager::Open {
                my_state,
                their_state,
                pending_rec,
            } => {
                let withdraw = pending_rec.clone();
                *pending_rec = 0.into();
                Ok(withdraw)
            }
            // TODO: Handle close and dispute
            _ => Ok(0.into()),
        }
    }

    /// This sums up the pending amount and returns a channel update
    pub fn create_payment(&mut self) -> Result<UpdateTx, Error> {
        match self {
            ChannelManager::Open {
                my_state,
                their_state,
                pending_rec,
            } => {
                let mut state = my_state.clone();

                state.nonce += 1.into();

                Ok(state.create_update())
            }
            // TODO: Handle close and dispute
            _ => bail!("can only pay in open state"),
        }
    }

    /// This is called by send_payment
    pub fn rec_payment(&mut self, update: UpdateTx) -> Result<UpdateTx, Error> {
        match self {
            ChannelManager::Open {
                my_state,
                their_state,
                pending_rec,
            } => {
                assert!(my_state.my_balance() <= their_state.my_balance());
                let pending_pay = their_state.my_balance() - my_state.my_balance();

                let our_prev_bal = their_state.my_balance().clone();
                their_state.apply_update(&update, true)?;
                let transfer = their_state.my_balance() - our_prev_bal;

                *pending_rec += transfer;

                *my_state = their_state.clone();

                assert!(&pending_pay <= their_state.my_balance());

                *my_state.my_balance_mut() -= pending_pay;
                *my_state.their_balance_mut() += pending_pay;
            }
            // TODO: Withdraw from close
            _ => bail!("can only pay in open state"),
        }

        Ok(self.create_payment()?)
    }

    /// This is called on the response to rec_payment
    pub fn rec_updated_state(
        &mut self,
        sent_update: UpdateTx,
        rec_update: UpdateTx,
    ) -> Result<(), Error> {
        match self {
            ChannelManager::Open {
                my_state,
                their_state,
                pending_rec,
            } => {
                assert!(my_state.my_balance() <= their_state.my_balance());
                let pending_pay = their_state.my_balance() - my_state.my_balance();

                let our_prev_bal = their_state.my_balance().clone();
                their_state.apply_update(&rec_update, false)?;
                let our_new_bal = their_state.my_balance();

                assert!(my_state.my_balance() <= their_state.my_balance());

                if our_prev_bal >= *our_new_bal {
                    let payment = our_prev_bal - our_new_bal;
                    // net effect was we payed them
                    if payment > pending_pay {
                        bail!("we paid them too much somehow");
                    }
                } else {
                    let payment = our_new_bal - our_prev_bal;

                    *pending_rec += payment;
                }

                Ok(())
            }
            // TODO: Handle close and dispute
            _ => bail!("can only pay in open state"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    /*
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

*/
    #[test]
    fn test_channel_manager_unidirectional_empty() {
        let (mut a, mut b) = ChannelManager::new_open_pair(100.into(), 100.into());

        let payment = a.create_payment().unwrap();

        b.rec_payment(payment.clone()).unwrap();
        let response = b.create_payment().unwrap();

        a.rec_updated_state(payment, response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 0.into());
        assert_eq!(b.withdraw().unwrap(), 0.into());
    }

    #[test]
    fn test_channel_manager_unidirectional() {
        let (mut a, mut b) = ChannelManager::new_open_pair(100.into(), 100.into());

        a.pay_counterparty(20.into());

        let payment = a.create_payment().unwrap();

        b.rec_payment(payment.clone()).unwrap();
        let response = b.create_payment().unwrap();

        a.rec_updated_state(payment, response).unwrap();

        assert_eq!(b.withdraw().unwrap(), 20.into());
        assert_eq!(b.withdraw().unwrap(), 0.into());
        assert_eq!(a.withdraw().unwrap(), 0.into());
    }

    #[test]
    fn test_channel_manager_bidirectional() {
        let (mut a, mut b) = ChannelManager::new_open_pair(100.into(), 100.into());

        // A -> B 5
        a.pay_counterparty(5.into()).unwrap();

        let payment = a.create_payment().unwrap();

        b.rec_payment(payment.clone()).unwrap();
        let response = b.create_payment().unwrap();

        a.rec_updated_state(payment, response).unwrap();

        // B -> A 3
        b.pay_counterparty(3.into()).unwrap();

        let payment = b.create_payment().unwrap();

        let response = a.rec_payment(payment.clone()).unwrap();

        b.rec_updated_state(payment, response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 3.into());
        assert_eq!(b.withdraw().unwrap(), 5.into());
    }

    #[test]
    fn test_channel_manager_bidirectional_race() {
        let (mut a, mut b) = ChannelManager::new_open_pair(100.into(), 100.into());

        // A -> B 3 and B -> A 5 at the same time
        a.pay_counterparty(3.into()).unwrap();
        b.pay_counterparty(5.into()).unwrap();

        let payment_a = a.create_payment().unwrap();
        let payment_b = b.create_payment().unwrap();

        let response_b = b.rec_payment(payment_a.clone()).unwrap();
        let response_a = a.rec_payment(payment_b.clone()).unwrap();

        a.rec_updated_state(payment_a, response_b).unwrap();
        b.rec_updated_state(payment_b, response_a).unwrap();

        // unraced request

        let payment = a.create_payment().unwrap();

        let response = b.rec_payment(payment.clone()).unwrap();

        a.rec_updated_state(payment, response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 5.into());
        assert_eq!(b.withdraw().unwrap(), 3.into());
    }

    #[test]
    fn test_channel_manager_bidirectional_race_resume() {
        let (mut a, mut b) = ChannelManager::new_open_pair(100.into(), 100.into());

        // A -> B 3 and B -> A 5 at the same time
        a.pay_counterparty(3.into()).unwrap();
        b.pay_counterparty(5.into()).unwrap();

        let payment_a = a.create_payment().unwrap();
        let payment_b = b.create_payment().unwrap();

        b.rec_payment(payment_a.clone()).unwrap();
        let response_b = b.create_payment().unwrap();
        a.rec_payment(payment_b.clone()).unwrap();
        let response_a = a.create_payment().unwrap();

        a.rec_updated_state(payment_a, response_b).unwrap();
        b.rec_updated_state(payment_b, response_a).unwrap();

        // A -> B 1
        a.pay_counterparty(1.into()).unwrap();

        let payment = a.create_payment().unwrap();

        b.rec_payment(payment.clone()).unwrap();
        let response = b.create_payment().unwrap();

        a.rec_updated_state(payment, response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 5.into());
        assert_eq!(b.withdraw().unwrap(), 4.into());
    }

    #[test]
    fn test_channel_manager_bidirectional_race_multi() {
        let (mut a, mut b) = ChannelManager::new_open_pair(100.into(), 100.into());

        // A -> B 1, B offline
        // A -> B 2, B -> A 4
        a.pay_counterparty(1.into()).unwrap();

        let payment_a1 = a.create_payment().unwrap();

        a.pay_counterparty(2.into()).unwrap();
        b.pay_counterparty(4.into()).unwrap();

        let payment_a2 = a.create_payment().unwrap();
        let payment_b = b.create_payment().unwrap();

        b.rec_payment(payment_a1.clone()).unwrap();
        let response_b1 = b.create_payment().unwrap();
        b.rec_payment(payment_a2.clone()).unwrap();
        let response_b2 = b.create_payment().unwrap();

        a.rec_payment(payment_b.clone()).unwrap();
        let response_a = a.create_payment().unwrap();

        a.rec_updated_state(payment_a1, response_b1).unwrap();
        a.rec_updated_state(payment_a2, response_b2).unwrap();
        b.rec_updated_state(payment_b, response_a).unwrap();

        // unraced request

        let payment = a.create_payment().unwrap();

        b.rec_payment(payment.clone()).unwrap();
        let response = b.create_payment().unwrap();

        a.rec_updated_state(payment, response).unwrap();

        let payment = b.create_payment().unwrap();

        a.rec_payment(payment.clone()).unwrap();
        let response = a.create_payment().unwrap();

        b.rec_updated_state(payment, response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 4.into());
        assert_eq!(b.withdraw().unwrap(), 3.into());
    }

    #[test]
    fn test_channel_manager_bidirectional_race_multi_resume() {
        let (mut a, mut b) = ChannelManager::new_open_pair(100.into(), 100.into());

        // A -> B 3, B no response
        // A -> B 3, B -> A 5
        a.pay_counterparty(3.into()).unwrap();

        let payment_a1 = a.create_payment().unwrap();

        a.pay_counterparty(3.into()).unwrap();
        b.pay_counterparty(5.into()).unwrap();

        let payment_a2 = a.create_payment().unwrap();
        let payment_b = b.create_payment().unwrap();

        b.rec_payment(payment_a1.clone()).unwrap();
        let _ = b.create_payment().unwrap();
        b.rec_payment(payment_a2.clone()).unwrap();
        let response_b2 = b.create_payment().unwrap();

        a.rec_payment(payment_b.clone()).unwrap();
        let response_a = a.create_payment().unwrap();

        a.rec_updated_state(payment_a2, response_b2).unwrap();
        b.rec_updated_state(payment_b, response_a).unwrap();

        // A -> B 10
        a.pay_counterparty(10.into()).unwrap();

        let payment = a.create_payment().unwrap();

        b.rec_payment(payment.clone()).unwrap();
        let response = b.create_payment().unwrap();

        a.rec_updated_state(payment, response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 5.into());
        assert_eq!(b.withdraw().unwrap(), 16.into());
    }
}
