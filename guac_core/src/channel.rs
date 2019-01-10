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
// direction, since if Alice sends Bob a message where s is lower than B(s), Bob will not accept
// it, and the system will halt.

// Packet loss in direction Alice -> Bob will only ever cause A(s) to be higher than B(s), since
// Alice has increased A(s) without Bob increasing B(s).

// Packet loss in direction Bob -> Alice will only ever cause B(s) to be lower than A(s), since
// Bob has decreased B(s) without Alice decreasing A(s).

use failure::Error;
use num256::Uint256;

use crate::types::{GuacError, UpdateTx};
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
    /// This prepares an UpdateTx which pays the counterparty, although it does not sign it.
    /// It also adjusts the stored balances and the sequence number.
    pub fn make_payment(
        &mut self,
        amount: Uint256,
        current_seq: Option<Uint256>,
    ) -> Result<UpdateTx, Error> {
        let sequence_number = if let Some(seq) = current_seq {
            seq + 1u64.into()
        } else {
            self.sequence_number.clone() + 1u64.into()
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

        self.balance_0 = balance_0.clone();
        self.balance_1 = balance_1.clone();
        self.sequence_number = sequence_number.clone();

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

    /// This checks the validity of a payment update (note that it does not verify signatures).
    /// If the sequence number is too low, it will return the current sequence number,
    /// which should be sent back to the counterparty so that they can try re-sending a correct
    /// payment. A successfully accepted payment results in a return value of Ok(None)
    /// This also adjusts `accrual` to measure how much the counterparty has paid us.
    /// Lost packets can result in an incorrect value of `accrual`.
    pub fn receive_payment(&mut self, update_tx: &UpdateTx) -> Result<Option<Uint256>, Error> {
        if update_tx.sequence_number <= self.sequence_number {
            return Ok(Some(self.sequence_number.clone()));
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
            != (my_balance.clone() + their_balance.clone())
        {
            return Err(GuacError::Forbidden {
                message: "Total amount in channel does not stay the same".into(),
            }
            .into());
        }

        if my_balance.clone() < my_old_balance.clone() {
            return Err(GuacError::Forbidden {
                message: "This reduces my balance".into(),
            }
            .into());
        }

        self.balance_0 = update_tx.balance_0.clone();
        self.balance_1 = update_tx.balance_1.clone();
        self.sequence_number = update_tx.sequence_number.clone();
        self.accrual = self.accrual.clone() + (my_balance - my_old_balance);

        Ok(None)
    }

    /// This checks the accrual. Accrual is a counter of all the payments that we have received
    /// from the counterparty. Packet loss of payments from us to the counterparty can result in
    /// this returning an innacurate value. If Alice tries to pay Bob 5, but the payment gets lost,
    /// when Bob sends Alice a payment for 5, it will appear to Alice as if Bob was paying her 10,
    /// since she doesn't know that he didn't get her payment.
    ///
    /// Also, more intuitively, Bob will not
    /// have gotten Alice's payment, so his accrual will be lower than it would be if he had. In
    /// many cases this would actually be the more serious problem.
    ///
    /// We (Althea) are not doing anything about this right now, since a solution would boil down to
    /// implementing a reliable transport, and there are other mechanisms to correct for these
    /// types of errors elsewhere in our codebase.
    pub fn check_accrual(&mut self) -> Uint256 {
        let accrual = self.accrual.clone();

        self.accrual = 0u64.into();

        accrual
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_channel() -> Channel {
        Channel {
            channel_id: [0; 32],
            sequence_number: 0u64.into(),
            balance_0: 0u64.into(),
            balance_1: 0u64.into(),
            accrual: 0u64.into(),
            i_am_0: false,
        }
    }

    #[test]
    fn test_unidirectional_empty() {
        let mut a = Channel {
            i_am_0: true,
            ..default_channel()
        };
        let mut b = Channel {
            i_am_0: false,
            ..default_channel()
        };

        let update = a.make_payment(0u64.into(), None).unwrap();

        b.receive_payment(&update).unwrap();

        assert!(b.check_accrual() == 0u64.into());
    }

    #[test]
    fn test_unidirectional_simple() {
        let mut a = Channel {
            i_am_0: true,
            balance_0: 10u64.into(),
            balance_1: 10u64.into(),
            ..default_channel()
        };
        let mut b = Channel {
            i_am_0: false,
            balance_0: 10u64.into(),
            balance_1: 10u64.into(),
            ..default_channel()
        };

        let update = a.make_payment(5u64.into(), None).unwrap();

        b.receive_payment(&update).unwrap();

        assert_eq!(
            a,
            Channel {
                i_am_0: true,
                balance_0: 5u64.into(),
                balance_1: 15u64.into(),
                sequence_number: 1u64.into(),
                ..default_channel()
            },
            "check a"
        );
        assert_eq!(
            b,
            Channel {
                i_am_0: false,
                balance_0: 5u64.into(),
                balance_1: 15u64.into(),
                sequence_number: 1u64.into(),
                accrual: 5u64.into(),
                ..default_channel()
            },
            "check b"
        );
        assert_eq!(b.check_accrual(), 5u64.into(), "check accrual");
        assert_eq!(b.check_accrual(), 0u64.into(), "check accrual reset");
    }

    #[test]
    fn test_bidirectional_simple() {
        let mut a = Channel {
            i_am_0: true,
            balance_0: 10u64.into(),
            balance_1: 10u64.into(),
            ..default_channel()
        };
        let mut b = Channel {
            i_am_0: false,
            balance_0: 10u64.into(),
            balance_1: 10u64.into(),
            ..default_channel()
        };

        let update = a.make_payment(5u64.into(), None).unwrap();

        b.receive_payment(&update).unwrap();

        let update = b.make_payment(6u64.into(), None).unwrap();

        a.receive_payment(&update).unwrap();

        assert_eq!(
            a,
            Channel {
                i_am_0: true,
                balance_0: 11u64.into(),
                balance_1: 9u64.into(),
                sequence_number: 2u64.into(),
                accrual: 6u64.into(),
                ..default_channel()
            },
            "check a"
        );
        assert_eq!(
            b,
            Channel {
                i_am_0: false,
                balance_0: 11u64.into(),
                balance_1: 9u64.into(),
                sequence_number: 2u64.into(),
                accrual: 5u64.into(),
                ..default_channel()
            },
            "check b"
        );
        assert_eq!(b.check_accrual(), 5u64.into(), "check accrual");
        assert_eq!(a.check_accrual(), 6u64.into(), "check accrual");
    }

    /// This test has A make a payment, then B loses a payment.
    /// After this, B receives A's payment.
    #[test]
    fn test_bidirectional_packet_loss() {
        let mut a = Channel {
            i_am_0: true,
            balance_0: 100u64.into(),
            balance_1: 100u64.into(),
            ..default_channel()
        };
        let mut b = Channel {
            i_am_0: false,
            balance_0: 100u64.into(),
            balance_1: 100u64.into(),
            ..default_channel()
        };

        let a_to_b_1 = a.make_payment(5u64.into(), None).unwrap();

        let _ = b.make_payment(5u64.into(), None).unwrap();
        let _ = b.make_payment(5u64.into(), None).unwrap();

        let current_seq = b.receive_payment(&a_to_b_1).unwrap().unwrap();
        let a_to_b_2 = a.make_payment(0u64.into(), Some(current_seq)).unwrap();

        if b.receive_payment(&a_to_b_2).unwrap().is_some() {
            panic!("should not return a sequence number")
        }

        assert_eq!(
            a,
            Channel {
                i_am_0: true,
                balance_0: 95u64.into(),
                balance_1: 105u64.into(),
                sequence_number: 3u64.into(),
                accrual: 0u64.into(),
                ..default_channel()
            },
            "check a"
        );
        assert_eq!(
            b,
            Channel {
                i_am_0: false,
                balance_0: 95u64.into(),
                balance_1: 105u64.into(),
                sequence_number: 3u64.into(),
                // This accrual is wrong because of the two lost packets but we don't care
                accrual: 15u64.into(),
                ..default_channel()
            },
            "check b"
        );
    }
}
