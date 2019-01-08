// Proof of balance liveness

// Consider a system where two nodes, Bob and Alice, are maintaining an integer s between them.
// Each node has stored s in its local memory. We will refer to Alice's copy of s as A(s).
// We will refer to Bob's copy of s as B(s).

// Bob and Alice may periodically send each other messages updating the value of s. Alice will
// only ever send a message increasing s, and Bob will only ever send a message decreasing s.

// When sending such a message, the sender will update their stored value of s. When receiving
// such a message, the receiver will also update their stored value of s. Packet loss may
// occur, such that A(s) and B(s) become unsyncronized.

// We would like to prove that A(s) will never be lower than B(s) due to packet loss in either
// direction, since if Alice sends Bob a message where s is lower than B(s),Bob will not accept
// it, and the system will halt.

// Packet loss in direction Alice -> Bob will only ever cause A(s) to be higher than B(s), since
// Alice has increased A(s) without Bob increasing B(s).

// Packet loss in direction Bob -> Alice will only ever cause B(s) to be lower than A(s), since
// Bob has decreased B(s) without Alice decreasing A(s).

use failure::Error;
use num256::Uint256;

use crate::channel_client::types::{GuacError, UpdateTx};
use num::traits::ops::checked::CheckedSub;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Channel {
    pub channel_id: [u8; 32],
    pub sequence_number: Uint256,
    pub balance_0: Uint256,
    pub balance_1: Uint256,
    pub accrual: Uint256,
    pub i_am_0: bool,
}

impl Channel {
    pub fn make_payment(
        &mut self,
        amount: Uint256,
        correct_seq: Option<Uint256>,
    ) -> Result<UpdateTx, Error> {
        let sequence_number = if let Some(seq) = correct_seq {
            seq
        } else {
            self.sequence_number.clone() + 1u8.into()
        };

        let (my_balance, their_balance) = if self.i_am_0 {
            (self.balance_0.clone(), self.balance_1.clone())
        } else {
            (self.balance_1.clone(), self.balance_0.clone())
        };

        let my_balance = my_balance
            .checked_sub(&amount)
            .ok_or_else(|| GuacError::NotEnough {
                stuff: "money in channel.".to_string(),
            })?;

        let their_balance = their_balance + amount;

        let (balance_0, balance_1) = if self.i_am_0 {
            (my_balance, their_balance)
        } else {
            (their_balance, my_balance)
        };

        let update_tx = UpdateTx {
            channel_id: self.channel_id,
            sequence_number,
            balance_0,
            balance_1,
            signature_0: None,
            signature_1: None,
        };

        Ok(update_tx)
    }

    pub fn receive_payment(&mut self, update_tx: &UpdateTx) -> Result<(), Error> {
        if update_tx.sequence_number <= self.sequence_number {
            return Err(GuacError::UpdateTooOld().into());
        };

        let (my_old_balance, their_old_balance) = if self.i_am_0 {
            (self.balance_0.clone(), self.balance_1.clone())
        } else {
            (self.balance_1.clone(), self.balance_0.clone())
        };

        let (my_balance, their_balance) = if self.i_am_0 {
            (update_tx.balance_0.clone(), update_tx.balance_1.clone())
        } else {
            (update_tx.balance_1.clone(), update_tx.balance_0.clone())
        };

        if (my_old_balance.clone() + their_old_balance.clone())
            == (my_balance.clone() + their_balance.clone())
        {
            return Err(GuacError::Forbidden {
                message: "Total amount in channel does not stay the same".into(),
            }
            .into());
        }

        if my_balance.clone() >= my_old_balance.clone() {
            return Err(GuacError::Forbidden {
                message: "This is not a payment".into(),
            }
            .into());
        }

        self.balance_0 = update_tx.balance_0.clone();
        self.balance_1 = update_tx.balance_1.clone();
        self.sequence_number = update_tx.sequence_number.clone();
        self.accrual = self.accrual.clone() + (my_balance - my_old_balance);

        Ok(())
    }

    pub fn check_accrual(&mut self) -> Uint256 {
        let accrual = self.accrual.clone();

        self.accrual = 0u64.into();

        accrual
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_channel(channel_id: [u8; 32], i_am_0: bool) -> Channel {
        Channel {
            channel_id,
            sequence_number: 0u8.into(),
            balance_0: 0u8.into(),
            balance_1: 0u8.into(),
            accrual: 0u8.into(),
            i_am_0,
        }
    }

    #[test]
    fn test_unidirectional_empty() {
        let mut a = new_channel([0; 32], true);
        let mut b = new_channel([0; 32], false);

        let update = a.make_payment(0u8.into(), None).unwrap();

        b.receive_payment(&update).unwrap();

        assert!(b.check_accrual() == 0u8.into());
    }
}
