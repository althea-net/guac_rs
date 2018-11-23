use failure::Error;

use clarity::Address;
use num256::Uint256;

use channel_client::combined_state::CombinedState;
use channel_client::types::{ChannelState, UpdateTx};
use channel_client::Channel;
use std::ops::Add;

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub enum ChannelManager {
    New,
    Proposed {
        state: Channel,
        accepted: bool,
    },
    /// After counterparty accepts proposal, while blockchain tx to create channel is pending
    PendingCreation {
        state: Channel,
        pending_send: Uint256,
    },
    /// After we accepts proposal, while counterparty's blockchain tx to create channel is pending
    PendingOtherCreation {
        state: Channel,
        pending_send: Uint256,
    },
    /// After counterparty opened channel, we ran out of credit in channel, while our blockchain tx to join is pending
    PendingJoin {
        state: CombinedState,
        pending_send: Uint256,
    },
    /// For party(s) who is already in channel
    Joined {
        state: CombinedState,
    },
    /// For party who is not in channel (if there is one party not in channel)
    Open {
        state: CombinedState,
    },
    // TODO: close/dispute
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChannelManagerAction {
    // to blockchain
    SendNewChannelTransaction(Channel),
    SendChannelJoinTransaction(Channel),

    // to counterparty
    SendChannelProposal(Channel),
    SendChannelCreatedUpdate(Channel),
    SendUpdatedState(UpdateTx),

    None,
}

/// If we should accept their proposal
fn is_channel_acceptable(_state: &Channel) -> Result<bool, Error> {
    Ok(true)
}

impl ChannelManager {
    // does some sanity checks on our current state to ensure nothing dodgy happened/will happen
    fn sanity_check(&self, my_address: &Address) {
        trace!(
            "checking sanity of {:?}, my address: {:?}",
            self,
            my_address
        );
        match self {
            ChannelManager::Proposed { state, .. }
            | ChannelManager::PendingCreation { state, .. }
            | ChannelManager::PendingOtherCreation { state, .. } => {
                if state.is_a {
                    assert_eq!(&state.address_a, my_address);
                } else {
                    assert_eq!(&state.address_b, my_address);
                }
            }
            ChannelManager::PendingJoin { state, .. }
            | ChannelManager::Joined { state }
            | ChannelManager::Open { state } => {
                if state.my_state().is_a {
                    assert_eq!(state.their_state().is_a, true);
                    assert_eq!(&state.my_state().address_a, my_address);
                    assert_eq!(&state.their_state().address_a, my_address);
                } else {
                    assert_eq!(state.their_state().is_a, false);
                    assert_eq!(&state.my_state().address_b, my_address);
                    assert_eq!(&state.their_state().address_b, my_address);
                }
            }
            _ => {}
        }
    }

    // called periodically so ChannelManager can talk to the external world
    pub fn tick(
        &mut self,
        my_address: Address,
        their_address: Address,
    ) -> Result<ChannelManagerAction, Error> {
        self.sanity_check(&my_address);
        match self.clone() {
            ChannelManager::New | ChannelManager::Proposed { .. } => {
                // Will continue to propose channel until successful every tick
                self.propose_channel(my_address, their_address, 100_000_000_000_000u64.into()) // 0.0001ETH
            }
            ChannelManager::PendingOtherCreation { state, .. } => {
                assert_eq!(state.is_a, false);
                // twiddle our thumbs, they will tell us when the channel is created
                Ok(ChannelManagerAction::None)
            }
            ChannelManager::PendingCreation {
                state,
                pending_send,
            } => {
                // we wait for creation of our channel
                // TODO: actually poll for stuff
                assert_eq!(state.is_a, true);
                *self = ChannelManager::Joined {
                    state: CombinedState::new(&state),
                };
                self.sanity_check(&my_address);
                self.pay_counterparty(pending_send)?;
                Ok(ChannelManagerAction::SendChannelCreatedUpdate(state.swap()))
            }
            ChannelManager::PendingJoin {
                state,
                pending_send,
            } => {
                assert_eq!(state.my_state().is_a, false);
                assert_eq!(state.their_state().is_a, false);

                // TODO: actually poll for stuff
                let mut state = state.clone();

                *state.my_state_mut().my_deposit_mut() = 100_000_000_000_000u64.into();
                *state.my_state_mut().my_balance_mut() = state
                    .my_state_mut()
                    .my_balance_mut()
                    .clone()
                    .add(Uint256::from(100_000_000_000_000u64));

                *state.their_state_mut().my_deposit_mut() = 100_000_000_000_000u64.into();
                *state.their_state_mut().my_balance_mut() = state
                    .their_state_mut()
                    .my_balance_mut()
                    .clone()
                    .add(Uint256::from(100_000_000_000_000u64));

                // now we have balance in our channel, we can pay what we owe them
                state.pay_counterparty(pending_send)?;

                *self = ChannelManager::Joined {
                    state: state.clone(),
                };
                Ok(ChannelManagerAction::SendChannelJoinTransaction({
                    state.my_state().clone()
                }))
            }
            ChannelManager::Joined { state: _ } | ChannelManager::Open { state: _ } => Ok(
                ChannelManagerAction::SendUpdatedState(self.create_payment()?),
            ),
        }
    }

    fn propose_channel(
        &mut self,
        from: Address,
        to: Address,
        deposit: Uint256,
    ) -> Result<ChannelManagerAction, Error> {
        ensure!(from != to, "cannot pay to self");
        let ret;
        *self = match self {
            ChannelManager::New => {
                // TODO make the defaults configurable
                let proposal = Channel {
                    state: ChannelState::New(to.clone()),
                    address_a: from,
                    address_b: to.clone(),
                    deposit_a: deposit.clone(),
                    deposit_b: 0u32.into(),
                    challenge: 0u32.into(),
                    nonce: 0u32.into(),
                    close_time: 10u32.into(),
                    balance_a: deposit.clone(),
                    balance_b: 0u32.into(),
                    is_a: true,
                    url: String::new(),
                };
                ret = ChannelManagerAction::SendChannelProposal(proposal.swap());
                ChannelManager::Proposed {
                    accepted: false,
                    state: proposal.clone(),
                }
            }
            ChannelManager::Proposed {
                accepted: false,
                state,
            } => {
                ret = ChannelManagerAction::None;
                ChannelManager::Proposed {
                    accepted: false,
                    state: state.clone(),
                }
            }
            ChannelManager::Proposed {
                accepted: true,
                state,
            } => {
                ret = ChannelManagerAction::SendNewChannelTransaction(state.clone());
                assert_eq!(state.is_a, true);
                ChannelManager::PendingCreation {
                    state: state.clone(),
                    pending_send: 0u32.into(),
                }
            }
            _ => bail!("can only propose if in state Proposed"),
        };

        Ok(ret)
    }

    pub fn channel_created(&mut self, channel: &Channel, my_address: Address) -> Result<(), Error> {
        trace!("checking proposal {:?}", channel);
        self.sanity_check(&my_address);
        let mut channel = channel.clone();
        *self = match self {
            ChannelManager::Proposed { .. }
            | ChannelManager::PendingOtherCreation { .. }
            | ChannelManager::PendingCreation { .. } => {
                // TODO: verify it actually made it into the blockchain
                if is_channel_acceptable(&channel)? {
                    if channel.address_a == my_address {
                        // we created this transaction
                        channel.is_a = true;
                        ChannelManager::Joined {
                            state: CombinedState::new(&channel),
                        }
                    } else if channel.address_b == my_address {
                        // They created this transaction
                        channel.is_a = false;
                        ChannelManager::Open {
                            state: CombinedState::new(&channel),
                        }
                    } else {
                        bail!("This channel is not related to us")
                    }
                } else {
                    bail!("Unacceptable channel created")
                }
            }
            _ => bail!("Channel creation when not in proposed state"),
        };
        self.sanity_check(&my_address);
        Ok(())
    }

    pub fn proposal_result(&mut self, decision: bool, pending_send: Uint256) -> Result<(), Error> {
        *self = match self {
            ChannelManager::Proposed { accepted, state } => {
                if decision {
                    ChannelManager::PendingCreation {
                        state: state.clone(),
                        pending_send,
                    }
                } else {
                    ChannelManager::Proposed {
                        accepted: *accepted,
                        state: state.clone(),
                    }
                }
            }
            _ => bail!("cannot accept proposal if not in New or Proposed"),
        };

        Ok(())
    }

    pub fn check_proposal(&mut self, their_prop: &Channel) -> Result<bool, Error> {
        if is_channel_acceptable(&their_prop)? {
            match self.clone() {
                ChannelManager::Proposed {
                    accepted: false,
                    state,
                } => {
                    let our_prop = state;
                    assert_ne!(their_prop.address_a, our_prop.address_a);
                    // smallest address wins
                    if their_prop.address_a > our_prop.address_a {
                        trace!("our address is lower, rejecting");
                        Ok(false)
                    } else {
                        // use their proposal
                        *self = ChannelManager::Proposed {
                            accepted: true,
                            state: their_prop.clone(),
                        };
                        Ok(true)
                    }
                }
                // accept if new
                ChannelManager::New => {
                    assert_eq!(their_prop.is_a, false);
                    *self = ChannelManager::PendingOtherCreation {
                        state: their_prop.clone(),
                        pending_send: 0u32.into(),
                    };
                    Ok(true)
                }
                _ => {
                    trace!("cannot accept proposal if not in New or Proposed");
                    Ok(false)
                }
            }
        } else {
            trace!("Cannot accept proposal");
            Ok(false)
        }
    }

    /// called when counterparty joined channel
    pub fn channel_joined(&mut self, chan: &Channel) -> Result<(), Error> {
        trace!("counterparty joined channel");
        match self {
            ChannelManager::Joined { state } => {
                ensure!(
                    chan.address_a == state.my_state().address_a,
                    "Channel for wrong address"
                );
                ensure!(
                    chan.address_b == state.my_state().address_b,
                    "Channel for wrong address"
                );
                ensure!(
                    chan.state == state.my_state().state,
                    "Wrong channelID"
                );
                ensure!(
                    chan.challenge == state.my_state().challenge,
                    "Conflicting challenge period"
                );

                // we must be a because we joined, check our deposit stays the same
                ensure!(
                    chan.deposit_a == state.my_state().deposit_a,
                    "our deposit must stay constant"
                );

                ensure!(
                    state.my_state().deposit_b == 0u32.into(),
                    "Their deposit must be 0 to begin with"
                );

                ensure!(
                    state.their_state().deposit_b == 0u32.into(),
                    "Their deposit must be 0 to begin with"
                );

                *state.my_state_mut().their_deposit_mut() += chan.clone().deposit_b;
                *state.my_state_mut().their_balance_mut() += chan.clone().deposit_b;
                *state.their_state_mut().their_deposit_mut() += chan.clone().deposit_b;
                *state.their_state_mut().their_balance_mut() += chan.clone().deposit_b;
            }
            _ => bail!("must be in state joined before counterparty joins"),
        };
        trace!("counterparty joined successful");
        Ok(())
    }

    pub fn pay_counterparty(&mut self, amount: Uint256) -> Result<(), Error> {
        *self = match self {
            ChannelManager::Open { ref mut state } => {
                assert_eq!(state.my_state().is_a, false);
                assert_eq!(state.their_state().is_a, false);
                let overflow = state.pay_counterparty(amount)?;
                trace!("got overflow of {:?}", overflow);
                if overflow > 0u32.into() {
                    trace!("not enough to pay, joining channel");
                    ChannelManager::PendingJoin {
                        state: state.clone(),
                        pending_send: overflow,
                    }
                } else {
                    ChannelManager::Open {
                        state: state.clone(),
                    }
                }
            }
            ChannelManager::Joined { ref mut state } => {
                let overflow = state.pay_counterparty(amount)?;
                if overflow > 0u32.into() {
                    // TODO: Handle reopening channel
                    trace!("not enough money to pay them");
                    bail!("not enough money to pay them")
                } else {
                    ChannelManager::Joined {
                        state: state.clone(),
                    }
                }
            }
            // we can still actually pay when we are joining the channel
            ChannelManager::PendingJoin {
                ref mut state,
                ref pending_send,
            } => {
                let overflow = state.pay_counterparty(amount)?;

                let mut pending_send = pending_send.clone();
                if overflow > 0u32.into() {
                    pending_send += overflow.clone();
                }
                ChannelManager::PendingJoin {
                    state: state.clone(),
                    pending_send,
                }
            }
            // TODO: Handle close and dispute
            _ => bail!("Invalid state for payment"),
        };

        Ok(())
    }

    pub fn withdraw(&mut self) -> Result<Uint256, Error> {
        match self {
            ChannelManager::Open { ref mut state } | ChannelManager::Joined { ref mut state } => {
                Ok(state.withdraw()?)
            }
            // TODO: Handle close and dispute
            _ => Ok(0u32.into()),
        }
    }

    pub fn create_payment(&mut self) -> Result<UpdateTx, Error> {
        match self {
            ChannelManager::Open { ref mut state }
            | ChannelManager::Joined { ref mut state }
            | ChannelManager::PendingJoin { ref mut state, .. } => Ok(state.create_payment()?),
            // TODO: Handle close and dispute
            _ => bail!("we can only create payments in open or joined"),
        }
    }

    pub fn received_payment(&mut self, payment: &UpdateTx) -> Result<UpdateTx, Error> {
        trace!("received payment {:?} state {:?}", payment, self);
        match self {
            ChannelManager::Open { ref mut state }
            | ChannelManager::Joined { ref mut state }
            | ChannelManager::PendingJoin { ref mut state, .. } => Ok(state.rec_payment(payment)?),
            // TODO: Handle close and dispute
            _ => bail!("we can only receive payments in open or joined"),
        }
    }

    pub fn received_updated_state(&mut self, rec_update: &UpdateTx) -> Result<(), Error> {
        match self {
            ChannelManager::Open { ref mut state }
            | ChannelManager::Joined { ref mut state }
            | ChannelManager::PendingJoin { ref mut state, .. } => {
                Ok(state.received_updated_state(rec_update)?)
            }
            // TODO: Handle close and dispute
            _ => bail!("we can only receive updated state in open or joined"),
        }
    }

    pub fn channel_open_event(&mut self, channel_id: &Uint256) -> Result<(), Error> {
        trace!("Channel open event {:?} {:?}", *self, channel_id);
        match *self {
            ChannelManager::Proposed { ref mut state, .. }
            | ChannelManager::PendingCreation { ref mut state, .. }
            | ChannelManager::PendingOtherCreation { ref mut state, .. } => {
                ensure!(
                    state.channel_id.is_none(),
                    "Unable to handle channel open event twice"
                );
                state.channel_id = Some(channel_id.clone());
                Ok(())
            }
            ref cm => bail!("Unable to set channel id with a state of {:?}", cm),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_channel_opening() {
        let mut manager_a = ChannelManager::New;
        let mut manager_b = ChannelManager::New;

        let proposal = manager_a
            .propose_channel(
                "0x0000000000000000000000000000000000000001"
                    .parse()
                    .unwrap(),
                "0x0000000000000000000000000000000000000002"
                    .parse()
                    .unwrap(),
                "0x0000000000000000000000000000000000000064"
                    .parse()
                    .unwrap(),
            ).unwrap();

        let mut channel_prop = match proposal {
            ChannelManagerAction::SendChannelProposal(channel) => channel,
            _ => panic!("Wrong action returned"),
        };

        channel_prop.channel_id = Some(42u64.into());

        assert!(manager_b.check_proposal(&channel_prop).unwrap());
        manager_a.proposal_result(true, 0u64.into()).unwrap();
        manager_a.channel_open_event(&Uint256::from(42u64)).unwrap();

        let (channel_a, channel_b) = Channel::new_pair(42u64.into(), 100u32.into(), 0u32.into());

        assert_eq!(
            manager_a,
            ChannelManager::PendingCreation {
                state: channel_a.clone(),
                pending_send: 0u32.into(),
            }
        );

        assert_eq!(
            manager_b,
            ChannelManager::PendingOtherCreation {
                state: channel_b.clone(),
                pending_send: 0u32.into(),
            }
        )
    }

    #[test]
    fn test_channel_opening_race() {
        let mut manager_a = ChannelManager::New;
        let mut manager_b = ChannelManager::New;

        let proposal_a = manager_a
            .propose_channel(
                "0x0000000000000000000000000000000000000001"
                    .parse()
                    .unwrap(),
                "0x0000000000000000000000000000000000000002"
                    .parse()
                    .unwrap(),
                "0x0000000000000000000000000000000000000064"
                    .parse()
                    .unwrap(),
            ).unwrap();

        let channel_prop_a = match proposal_a {
            ChannelManagerAction::SendChannelProposal(channel) => channel,
            _ => panic!("Wrong action returned"),
        };

        let proposal_b = manager_b
            .propose_channel(
                "0x0000000000000000000000000000000000000002"
                    .parse()
                    .unwrap(),
                "0x0000000000000000000000000000000000000001"
                    .parse()
                    .unwrap(),
                "0x0000000000000000000000000000000000000064"
                    .parse()
                    .unwrap(),
            ).unwrap();

        let channel_prop_b = match proposal_b {
            ChannelManagerAction::SendChannelProposal(channel) => channel,
            _ => panic!("Wrong action returned"),
        };

        assert!(manager_b.check_proposal(&channel_prop_a).unwrap());
        assert!(!manager_a.check_proposal(&channel_prop_b).unwrap());
        manager_a.proposal_result(true, 0u64.into()).unwrap();
        manager_b.proposal_result(false, 0u64.into()).unwrap();

        assert_eq!(
            manager_a,
            ChannelManager::PendingCreation {
                state: channel_prop_a.swap(),
                pending_send: 0u32.into(),
            }
        );

        manager_a
            .channel_created(
                &channel_prop_a,
                "0x0000000000000000000000000000000000000001"
                    .parse()
                    .unwrap(),
            ).unwrap();
        manager_b
            .channel_created(
                &channel_prop_a,
                "0x0000000000000000000000000000000000000002"
                    .parse()
                    .unwrap(),
            ).unwrap();

        assert_eq!(
            manager_a,
            ChannelManager::Joined {
                state: CombinedState::new(&channel_prop_a.swap())
            }
        );

        assert_eq!(
            manager_b,
            ChannelManager::Open {
                state: CombinedState::new(&channel_prop_a)
            }
        )
    }
}
