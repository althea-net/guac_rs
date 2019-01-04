use failure::Error;

use num256::Uint256;

use crate::channel_client::types::{Channel, GuacError, UpdateTx};
use std::ops::{Add, Sub};

// TODO:
// - increment in create update. Only call it when needed
// - receive_payment_ack returns special error on old sequence number
// - receive_payment

/// A struct which represents the core payment logic/state of a payment channel. It contains both
/// our current state as well as the last confirmed state of our counterparty, which is used to
/// ensure multiple in flight payments will resolve successfully and requests can be lost in either
/// direction without losing money or losing track of how much they paid us/we are going to pay them
///
/// NOTE: In both states the is_a bool is constant, instead of being flipped on `their_state`
/// This is because the numerous `.my_...` and `.their_...` methods on the `Channel` structs rely on
/// that to work, and the code would be a lot more confusing if it was flipped
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct CombinedState {
    /// This represents our current state
    pub their_state: Channel,
    /// This represents the last confirmed state we have from them
    pub my_state: Channel,

    /// This represents the amount of money we have confirmed we will recieve from them, but have
    /// not been `withdraw`n yet
    pub pending_receive: Uint256,
}

impl CombinedState {
    pub fn new(channel: &Channel) -> CombinedState {
        CombinedState {
            their_state: channel.clone(),
            my_state: channel.clone(),
            pending_receive: 0u32.into(),
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

    /// Function to pay counterparty by updating our state. This doesn't actually create any state
    /// updates, mearly ensures that the next state update we create will give the counterparty the
    /// amount sent. This function returns the "overflow" (amount - current balance in channel) if
    /// we don't have enough monty in the channel
    pub fn make_payment(&mut self, amount: Uint256) -> Result<Uint256, Error> {
        if amount > *self.my_state.my_balance_mut() {
            // Figure out how much we will still owe them
            let remaining_amount = amount.clone() - self.my_state.my_balance().clone();

            // Add our entire balance to their balance
            *self.my_state.their_balance_mut() = self
                .my_state
                .their_balance_mut()
                .clone()
                .add(self.my_state.my_balance().clone());

            // Set our balance to zero
            *self.my_state.my_balance_mut() = 0u32.into();

            // Return how much we still owe them
            Ok(remaining_amount)
        } else {
            // Subtract amount from our balance
            *self.my_state.my_balance_mut() =
                self.my_state.my_balance_mut().clone().sub(amount.clone());

            // Add amount to their balance
            *self.my_state.their_balance_mut() = self
                .my_state
                .their_balance_mut()
                .clone()
                .add(amount.clone());
            Ok(0u32.into())
        }
    }

    pub fn withdraw(&mut self) -> Result<Uint256, Error> {
        let withdraw = self.pending_receive.clone();
        self.pending_receive = 0u32.into();
        Ok(withdraw)
    }

    /// This function creates a state update from our current state, which takes into account
    /// all the `make_payment`'s which have happened between the last invocation of this
    /// function
    pub fn create_update(&mut self) -> Result<UpdateTx, Error> {
        self.my_state.sequence_number += 1u8.into();
        self.their_state.sequence_number += 1u8.into();

        Ok(self.my_state.create_update())
    }

    /// This is what processes the `UpdateTx` created by the `create_update` on the counterparty.
    pub fn receive_payment(&mut self, update: &UpdateTx) -> Result<UpdateTx, Error> {
        trace!("applying update {:?} on top of {:?}", update, self);

        ensure!(
            self.my_state.my_balance() <= self.their_state.my_balance(),
            "Our state needs to be worse for us than their state"
        );

        // Pending_pay is what we think they think our balance is minus
        // what we think our balance is
        // pending_pay is the amount that we want to pay them but have not
        // yet sent them an update for.
        let pending_pay = self
            .their_state
            .my_balance()
            .clone()
            .sub(self.my_state.my_balance().clone());

        // by applying their state update on top of their state, we can know how much they are going
        // to pay us, if we didn't do any transactions

        // Save our previous balance in their state
        let our_prev_bal = self.their_state.my_balance().clone();

        //
        // self.their_state.apply_update(&update, false)?;

        if let Err(err) = self.their_state.apply_update(&update, false) {
            match err.downcast::<GuacError>()? {
                GuacError::UpdateTooOld() => {
                    return Ok(UpdateTx {
                        sequence_number: self.their_state.sequence_number.clone(),
                        ..update.clone()
                    });
                }
                guac_err => return Err(guac_err.into()),
            }
        }

        ensure!(
            *self.their_state.my_balance() >= our_prev_bal,
            "My balance needs to be bigger than our previous balance"
        );

        // How much they have paid us with this update
        let transfer = self
            .their_state
            .my_balance()
            .clone()
            .sub(our_prev_bal.clone());

        // Add that to the pending recieve
        self.pending_receive = self.pending_receive.clone().add(transfer.clone());

        // This essentially "rolls back" any payments we have done and they have
        // not acknowledged the update for
        self.my_state = self.their_state.clone();

        assert!(&pending_pay <= self.their_state.my_balance());

        // so here we put it back
        *self.my_state.my_balance_mut() = self
            .my_state
            .my_balance_mut()
            .clone()
            .sub(pending_pay.clone());
        *self.my_state.their_balance_mut() = self
            .my_state
            .their_balance_mut()
            .clone()
            .add(pending_pay.clone());

        Ok(self.create_update()?)
    }

    /// This is what processes the `UpdateTx` created by the `receive_payment` on the counterparty.
    pub fn receive_payment_ack(&mut self, rec_update: &UpdateTx) -> Result<(), Error> {
        ensure!(
            self.my_state.my_balance() <= self.their_state.my_balance(),
            "cannot take money our state: {:?}, their update {:?}",
            self,
            rec_update
        );

        // Saving what we intend to pay them but have not yet
        let pending_pay = self
            .their_state
            .my_balance()
            .clone()
            .sub(self.my_state.my_balance().clone());

        let our_prev_bal = self.their_state.my_balance().clone();
        self.their_state.apply_update(&rec_update, false)?;
        let our_new_bal = self.their_state.my_balance();

        assert!(self.my_state.my_balance() <= self.their_state.my_balance());

        if our_prev_bal >= *our_new_bal {
            // net effect was we payed them
            let payment = our_prev_bal.sub(our_new_bal.clone());
            if payment > pending_pay {
                bail!("we paid them too much somehow");
            }
        } else {
            let payment = our_new_bal.clone().sub(our_prev_bal.clone());
            self.pending_receive = self.pending_receive.clone().add(payment.clone());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_pair(balance_0: Uint256, balance_1: Uint256) -> (CombinedState, CombinedState) {
        // let (channel_a, channel_b) = Channel::new_pair(42u64.into(), deposit_a, deposit_b);

        let channel_a = Channel {
            channel_id: [0u8; 32],
            address_0: "0x0000000000000000000000000000000000000001"
                .parse()
                .unwrap(),
            address_1: "0x0000000000000000000000000000000000000002"
                .parse()
                .unwrap(),
            settling_period_length: 0u64.into(),
            settling_period_end: 0u64.into(),
            settling_period_started: false,
            sequence_number: 0u64.into(),
            balance_0: balance_0.clone(),
            balance_1: balance_1.clone(),
            total_balance: balance_0.add(balance_1),
            i_am_0: true,
        };

        let channel_b = Channel {
            i_am_0: false,
            ..channel_a.clone()
        };

        (
            CombinedState::new(&channel_a),
            CombinedState::new(&channel_b),
        )
    }

    #[test]
    fn test_unidirectional_empty() {
        let (mut a, mut b) = new_pair(100u32.into(), 100u32.into());

        let payment = a.create_update().unwrap();

        b.receive_payment(&payment).unwrap();
        let response = b.create_update().unwrap();

        a.receive_payment_ack(&response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 0u32.into());
        assert_eq!(b.withdraw().unwrap(), 0u32.into());
    }

    #[test]
    fn test_unidirectional_overpay() {
        let (mut a, mut b) = new_pair(100u32.into(), 100u32.into());

        let overflow = a.make_payment(150u32.into()).unwrap();

        assert_eq!(overflow, 50u32.into());

        let payment = a.create_update().unwrap();

        b.receive_payment(&payment).unwrap();
        let response = b.create_update().unwrap();

        a.receive_payment_ack(&response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 0u32.into());
        assert_eq!(b.withdraw().unwrap(), 100u32.into());
    }

    #[test]
    fn test_unidirectional() {
        let (mut a, mut b) = new_pair(100u32.into(), 100u32.into());

        a.make_payment(20u32.into()).unwrap();

        let payment = a.create_update().unwrap();

        b.receive_payment(&payment).unwrap();
        let response = b.create_update().unwrap();

        a.receive_payment_ack(&&response).unwrap();

        assert_eq!(b.withdraw().unwrap(), 20u32.into());
        assert_eq!(b.withdraw().unwrap(), 0u32.into());
        assert_eq!(a.withdraw().unwrap(), 0u32.into());
    }

    #[test]
    fn test_bidirectional_simple() {
        let (mut a, mut b) = new_pair(100u32.into(), 100u32.into());

        // A -> B 5
        a.make_payment(5u32.into()).unwrap();

        let payment = a.create_update().unwrap();

        b.receive_payment(&payment).unwrap();
        let response = b.create_update().unwrap();

        a.receive_payment_ack(&response).unwrap();

        // B -> A 3
        b.make_payment(3u32.into()).unwrap();

        let payment = b.create_update().unwrap();

        let response = a.receive_payment(&payment).unwrap();

        b.receive_payment_ack(&response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 3u32.into());
        assert_eq!(b.withdraw().unwrap(), 5u32.into());
    }

    #[test]
    fn test_bidirectional_race() {
        let (mut a, mut b) = new_pair(100u32.into(), 100u32.into());

        // A -> B 3 and B -> A 5 at the same time
        a.make_payment(3u32.into()).unwrap();
        b.make_payment(5u32.into()).unwrap();

        let payment_a = a.create_update().unwrap();
        let payment_b = b.create_update().unwrap();

        let response_b = b.receive_payment(&payment_a).unwrap();
        let response_a = a.receive_payment(&payment_b).unwrap();

        a.receive_payment_ack(&response_b).unwrap();
        b.receive_payment_ack(&response_a).unwrap();

        // unraced request

        let payment = a.create_update().unwrap();

        let response = b.receive_payment(&payment).unwrap();

        a.receive_payment_ack(&response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 5u32.into());
        assert_eq!(b.withdraw().unwrap(), 3u32.into());
    }

    #[test]
    fn test_bidirectional_race_resume() {
        let (mut a, mut b) = new_pair(100u32.into(), 100u32.into());

        // A -> B 3 and B -> A 5 at the same time
        a.make_payment(3u32.into()).unwrap();
        b.make_payment(5u32.into()).unwrap();

        let payment_a = a.create_update().unwrap();
        let payment_b = b.create_update().unwrap();

        b.receive_payment(&payment_a).unwrap();
        let response_b = b.create_update().unwrap();
        a.receive_payment(&payment_b).unwrap();
        let response_a = a.create_update().unwrap();

        a.receive_payment_ack(&response_b).unwrap();
        b.receive_payment_ack(&response_a).unwrap();

        // A -> B 1
        a.make_payment(1u8.into()).unwrap();

        let payment = a.create_update().unwrap();

        b.receive_payment(&payment).unwrap();
        let response = b.create_update().unwrap();

        a.receive_payment_ack(&response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 5u32.into());
        assert_eq!(b.withdraw().unwrap(), 4u32.into());
    }

    #[test]
    fn test_bidirectional_race_multi() {
        let (mut a, mut b) = new_pair(100u32.into(), 100u32.into());

        // A -> B 1, B offline
        // A -> B 2, B -> A 4
        a.make_payment(1u8.into()).unwrap();

        let payment_a1 = a.create_update().unwrap();

        a.make_payment(2u32.into()).unwrap();
        b.make_payment(4u32.into()).unwrap();

        let payment_a2 = a.create_update().unwrap();
        let payment_b = b.create_update().unwrap();

        b.receive_payment(&payment_a1).unwrap();
        let response_b1 = b.create_update().unwrap();
        b.receive_payment(&payment_a2).unwrap();
        let response_b2 = b.create_update().unwrap();

        a.receive_payment(&payment_b).unwrap();
        let response_a = a.create_update().unwrap();

        a.receive_payment_ack(&response_b1).unwrap();
        a.receive_payment_ack(&response_b2).unwrap();
        b.receive_payment_ack(&response_a).unwrap();

        // unraced request

        let payment = a.create_update().unwrap();

        b.receive_payment(&payment).unwrap();
        let response = b.create_update().unwrap();

        a.receive_payment_ack(&response).unwrap();

        let payment = b.create_update().unwrap();

        a.receive_payment(&payment).unwrap();
        let response = a.create_update().unwrap();

        b.receive_payment_ack(&response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 4u32.into());
        assert_eq!(b.withdraw().unwrap(), 3u32.into());
    }

    #[test]
    fn test_bidirectional_race_multi_resume() {
        let (mut a, mut b) = new_pair(100u32.into(), 100u32.into());

        // A -> B 3, B no response
        // A -> B 3, B -> A 5
        a.make_payment(3u32.into()).unwrap();

        let payment_a1 = a.create_update().unwrap();

        a.make_payment(3u32.into()).unwrap();
        b.make_payment(5u32.into()).unwrap();

        let payment_a2 = a.create_update().unwrap();
        let payment_b = b.create_update().unwrap();

        b.receive_payment(&payment_a1).unwrap();
        let _ = b.create_update().unwrap();
        b.receive_payment(&payment_a2).unwrap();
        let response_b2 = b.create_update().unwrap();

        a.receive_payment(&payment_b).unwrap();
        let response_a = a.create_update().unwrap();

        a.receive_payment_ack(&response_b2).unwrap();
        b.receive_payment_ack(&response_a).unwrap();

        // A -> B 10
        a.make_payment(10u32.into()).unwrap();

        let payment = a.create_update().unwrap();

        b.receive_payment(&payment).unwrap();
        let response = b.create_update().unwrap();

        a.receive_payment_ack(&response).unwrap();

        assert_eq!(a.withdraw().unwrap(), 5u32.into());
        assert_eq!(b.withdraw().unwrap(), 16u32.into());
    }
}
