use failure::Error;

use ethereum_types::U256;

use channel_client::types::{Channel, UpdateTx};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CombinedState {
    their_state: Channel,
    my_state: Channel,

    pending_receive: U256,
}

impl CombinedState {
    pub fn new(channel: &Channel) -> CombinedState {
        CombinedState {
            their_state: channel.clone(),
            my_state: channel.clone(),
            pending_receive: 0.into(),
        }
    }

    pub fn my_state(&self) -> &Channel {
        &self.my_state
    }

    pub fn my_state_mut(&mut self) -> &mut Channel {
        &mut self.my_state
    }

    pub fn their_state(&self) -> &Channel {
        &self.their_state
    }

    pub fn their_state_mut(&mut self) -> &mut Channel {
        &mut self.their_state
    }

    /// Function to pay counterparty, doesn't actually send anything, returning the "overflow" if we
    /// don't have enough monty in the channel
    pub fn pay_counterparty(&mut self, amount: U256) -> Result<U256, Error> {
        if amount > *self.my_state.my_balance_mut() {
            let remaining_amount = amount - *self.my_state.my_balance();

            *self.my_state.their_balance_mut() += *self.my_state.my_balance();
            *self.my_state.my_balance_mut() = 0.into();

            Ok(remaining_amount)
        } else {
            *self.my_state.my_balance_mut() -= amount;
            *self.my_state.their_balance_mut() += amount;
            Ok(0.into())
        }
    }

    pub fn withdraw(&mut self) -> Result<U256, Error> {
        let withdraw = self.pending_receive;
        self.pending_receive = 0.into();
        Ok(withdraw)
    }

    /// This sums up the pending amount and returns a channel update
    pub fn create_payment(&mut self) -> Result<UpdateTx, Error> {
        let mut state = self.my_state.clone();

        state.nonce += 1.into();

        Ok(state.create_update())
    }

    /// This is called by send_payment
    pub fn rec_payment(&mut self, update: &UpdateTx) -> Result<UpdateTx, Error> {
        trace!("applying update {:?} on top of {:?}", update, self);

        ensure!(
            self.my_state.my_balance() <= self.their_state.my_balance(),
            "cannot take money"
        );
        let pending_pay = self.their_state.my_balance() - self.my_state.my_balance();

        let our_prev_bal = self.their_state.my_balance().clone();
        self.their_state.apply_update(&update, false)?;

        ensure!(
            *self.their_state.my_balance() >= our_prev_bal,
            "My balance needs to be bigger than our previous balance"
        );

        let transfer = self.their_state.my_balance() - our_prev_bal;

        self.pending_receive += transfer;

        self.my_state = self.their_state.clone();

        assert!(&pending_pay <= self.their_state.my_balance());

        *self.my_state.my_balance_mut() -= pending_pay;
        *self.my_state.their_balance_mut() += pending_pay;

        Ok(self.create_payment()?)
    }

    /// This is called on the response to rec_payment
    pub fn received_updated_state(&mut self, rec_update: &UpdateTx) -> Result<(), Error> {
        ensure!(
            self.my_state.my_balance() <= self.their_state.my_balance(),
            "cannot take money our state: {:?}, their update {:?}",
            self,
            rec_update
        );
        let pending_pay = self.their_state.my_balance() - self.my_state.my_balance();

        let our_prev_bal = self.their_state.my_balance().clone();
        self.their_state.apply_update(&rec_update, false)?;
        let our_new_bal = self.their_state.my_balance();

        assert!(self.my_state.my_balance() <= self.their_state.my_balance());

        if our_prev_bal >= *our_new_bal {
            let payment = our_prev_bal - our_new_bal;
            // net effect was we payed them
            if payment > pending_pay {
                bail!("we paid them too much somehow");
            }
        } else {
            let payment = our_new_bal - our_prev_bal;
            self.pending_receive += payment;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_pair(deposit_a: U256, deposit_b: U256) -> (CombinedState, CombinedState) {
        let (channel_a, channel_b) = Channel::new_pair(deposit_a, deposit_b);
        (
            CombinedState::new(&channel_a),
            CombinedState::new(&channel_b),
        )
    }

    #[test]
    fn test_channel_manager_unidirectional_empty() {
        let (mut a, mut b) = new_pair(100.into(), 100.into());

        let payment = a.create_payment().unwrap();

        b.rec_payment(&payment).unwrap();
        let response = b.create_payment().unwrap();

        a.received_updated_state(&response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 0.into());
        assert_eq!(b.withdraw().unwrap(), 0.into());
    }

    #[test]
    fn test_channel_manager_unidirectional_overpay() {
        let (mut a, mut b) = new_pair(100.into(), 100.into());

        let overflow = a.pay_counterparty(150.into()).unwrap();

        assert_eq!(overflow, 50.into());

        let payment = a.create_payment().unwrap();

        b.rec_payment(&payment).unwrap();
        let response = b.create_payment().unwrap();

        a.received_updated_state(&response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 0.into());
        assert_eq!(b.withdraw().unwrap(), 100.into());
    }

    #[test]
    fn test_channel_manager_unidirectional() {
        let (mut a, mut b) = new_pair(100.into(), 100.into());

        a.pay_counterparty(20.into()).unwrap();

        let payment = a.create_payment().unwrap();

        b.rec_payment(&payment).unwrap();
        let response = b.create_payment().unwrap();

        a.received_updated_state(&&response).unwrap();

        assert_eq!(b.withdraw().unwrap(), 20.into());
        assert_eq!(b.withdraw().unwrap(), 0.into());
        assert_eq!(a.withdraw().unwrap(), 0.into());
    }

    #[test]
    fn test_channel_manager_bidirectional() {
        let (mut a, mut b) = new_pair(100.into(), 100.into());

        // A -> B 5
        a.pay_counterparty(5.into()).unwrap();

        let payment = a.create_payment().unwrap();

        b.rec_payment(&payment).unwrap();
        let response = b.create_payment().unwrap();

        a.received_updated_state(&response).unwrap();

        // B -> A 3
        b.pay_counterparty(3.into()).unwrap();

        let payment = b.create_payment().unwrap();

        let response = a.rec_payment(&payment).unwrap();

        b.received_updated_state(&response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 3.into());
        assert_eq!(b.withdraw().unwrap(), 5.into());
    }

    #[test]
    fn test_channel_manager_bidirectional_race() {
        let (mut a, mut b) = new_pair(100.into(), 100.into());

        // A -> B 3 and B -> A 5 at the same time
        a.pay_counterparty(3.into()).unwrap();
        b.pay_counterparty(5.into()).unwrap();

        let payment_a = a.create_payment().unwrap();
        let payment_b = b.create_payment().unwrap();

        let response_b = b.rec_payment(&payment_a).unwrap();
        let response_a = a.rec_payment(&payment_b).unwrap();

        a.received_updated_state(&response_b).unwrap();
        b.received_updated_state(&response_a).unwrap();

        // unraced request

        let payment = a.create_payment().unwrap();

        let response = b.rec_payment(&payment).unwrap();

        a.received_updated_state(&response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 5.into());
        assert_eq!(b.withdraw().unwrap(), 3.into());
    }

    #[test]
    fn test_channel_manager_bidirectional_race_resume() {
        let (mut a, mut b) = new_pair(100.into(), 100.into());

        // A -> B 3 and B -> A 5 at the same time
        a.pay_counterparty(3.into()).unwrap();
        b.pay_counterparty(5.into()).unwrap();

        let payment_a = a.create_payment().unwrap();
        let payment_b = b.create_payment().unwrap();

        b.rec_payment(&payment_a).unwrap();
        let response_b = b.create_payment().unwrap();
        a.rec_payment(&payment_b).unwrap();
        let response_a = a.create_payment().unwrap();

        a.received_updated_state(&response_b).unwrap();
        b.received_updated_state(&response_a).unwrap();

        // A -> B 1
        a.pay_counterparty(1.into()).unwrap();

        let payment = a.create_payment().unwrap();

        b.rec_payment(&payment).unwrap();
        let response = b.create_payment().unwrap();

        a.received_updated_state(&response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 5.into());
        assert_eq!(b.withdraw().unwrap(), 4.into());
    }

    #[test]
    fn test_channel_manager_bidirectional_race_multi() {
        let (mut a, mut b) = new_pair(100.into(), 100.into());

        // A -> B 1, B offline
        // A -> B 2, B -> A 4
        a.pay_counterparty(1.into()).unwrap();

        let payment_a1 = a.create_payment().unwrap();

        a.pay_counterparty(2.into()).unwrap();
        b.pay_counterparty(4.into()).unwrap();

        let payment_a2 = a.create_payment().unwrap();
        let payment_b = b.create_payment().unwrap();

        b.rec_payment(&payment_a1).unwrap();
        let response_b1 = b.create_payment().unwrap();
        b.rec_payment(&payment_a2).unwrap();
        let response_b2 = b.create_payment().unwrap();

        a.rec_payment(&payment_b).unwrap();
        let response_a = a.create_payment().unwrap();

        a.received_updated_state(&response_b1).unwrap();
        a.received_updated_state(&response_b2).unwrap();
        b.received_updated_state(&response_a).unwrap();

        // unraced request

        let payment = a.create_payment().unwrap();

        b.rec_payment(&payment).unwrap();
        let response = b.create_payment().unwrap();

        a.received_updated_state(&response).unwrap();

        let payment = b.create_payment().unwrap();

        a.rec_payment(&payment).unwrap();
        let response = a.create_payment().unwrap();

        b.received_updated_state(&response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 4.into());
        assert_eq!(b.withdraw().unwrap(), 3.into());
    }

    #[test]
    fn test_channel_manager_bidirectional_race_multi_resume() {
        let (mut a, mut b) = new_pair(100.into(), 100.into());

        // A -> B 3, B no response
        // A -> B 3, B -> A 5
        a.pay_counterparty(3.into()).unwrap();

        let payment_a1 = a.create_payment().unwrap();

        a.pay_counterparty(3.into()).unwrap();
        b.pay_counterparty(5.into()).unwrap();

        let payment_a2 = a.create_payment().unwrap();
        let payment_b = b.create_payment().unwrap();

        b.rec_payment(&payment_a1).unwrap();
        let _ = b.create_payment().unwrap();
        b.rec_payment(&payment_a2).unwrap();
        let response_b2 = b.create_payment().unwrap();

        a.rec_payment(&payment_b).unwrap();
        let response_a = a.create_payment().unwrap();

        a.received_updated_state(&response_b2).unwrap();
        b.received_updated_state(&response_a).unwrap();

        // A -> B 10
        a.pay_counterparty(10.into()).unwrap();

        let payment = a.create_payment().unwrap();

        b.rec_payment(&payment).unwrap();
        let response = b.create_payment().unwrap();

        a.received_updated_state(&response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 5.into());
        assert_eq!(b.withdraw().unwrap(), 16.into());
    }
}
