use failure::Error;

use althea_types::{Bytes32, EthAddress, EthPrivateKey, EthSignature};
use ethereum_types::U256;

use channel_client::combined_state::CombinedState;
use channel_client::types::{ChannelStatus, UpdateTx};
use channel_client::Channel;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ChannelManager {
    New,
    Proposed {
        state: Channel,
        accepted: bool,
    },
    /// After counterparty accepts proposal, while blockchain tx to create channel is pending
    PendingCreation {
        state: Channel,
        pending_send: U256,
    },
    /// After counterparty opened channel, we ran out of credit in channel, while our blockchain tx to join is pending
    PendingJoin {
        state: CombinedState,
        pending_send: U256,
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
fn is_channel_acceptable(state: &Channel) -> Result<bool, Error> {
    Ok(true)
}

impl ChannelManager {
    fn new() -> ChannelManager {
        ChannelManager::New
    }

    // called periodically so ChannelManager can talk to the external world
    pub fn tick(
        &mut self,
        my_address: EthAddress,
        their_address: EthAddress,
    ) -> Result<ChannelManagerAction, Error> {
        match self.clone() {
            ChannelManager::New | ChannelManager::Proposed { .. } => {
                self.propose_channel(my_address, their_address, 1000.into())
            }
            ChannelManager::PendingCreation {
                state,
                pending_send,
            } => {
                // we wait for creation of our channel
                // TODO: actually poll for stuff
                *self = ChannelManager::Joined {
                    state: CombinedState::new(&state),
                };
                self.pay_counterparty(pending_send);
                Ok(ChannelManagerAction::SendChannelCreatedUpdate(state))
            }
            ChannelManager::PendingJoin {
                state,
                pending_send,
            } => {
                // TODO: actually poll for stuff
                *self = ChannelManager::Joined { state };
                self.pay_counterparty(pending_send);
                Ok(ChannelManagerAction::None)
            }
            ChannelManager::Joined { state } | ChannelManager::Open { state } => Ok(
                ChannelManagerAction::SendUpdatedState(self.create_payment()?),
            ),
        }
    }

    fn propose_channel(
        &mut self,
        from: EthAddress,
        to: EthAddress,
        deposit: U256,
    ) -> Result<ChannelManagerAction, Error> {
        let ret;
        *self = match self {
            ChannelManager::New => {
                // TODO make the defaults configurable
                let proposal = Channel {
                    channel_id: 0.into(),
                    address_a: from,
                    address_b: to,
                    channel_status: ChannelStatus::Joined,
                    deposit_a: deposit,
                    deposit_b: 0.into(),
                    challenge: 0.into(),
                    nonce: 0.into(),
                    close_time: 10.into(),
                    balance_a: deposit,
                    balance_b: 0.into(),
                    is_a: true,
                };
                ret = ChannelManagerAction::SendChannelProposal(proposal.clone());
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
                ChannelManager::PendingCreation {
                    state: state.clone(),
                    pending_send: 0.into(),
                }
            }
            _ => bail!("can only propose if in state Proposed"),
        };

        Ok(ret)
    }

    pub fn channel_created(
        &mut self,
        channel: &Channel,
        our_address: EthAddress,
    ) -> Result<(), Error> {
        *self = match self {
            ChannelManager::Proposed { .. } | ChannelManager::PendingCreation { .. } => {
                // TODO: verify it actually made it into the blockchain
                if is_channel_acceptable(&channel)? {
                    if channel.address_a == our_address {
                        // we created this transaction
                        ChannelManager::Joined {
                            state: CombinedState::new(&channel),
                        }
                    } else {
                        // They created this transaction
                        ChannelManager::Open {
                            state: CombinedState::new(&channel.swap()),
                        }
                    }
                } else {
                    bail!("Unacceptable channel created")
                }
            }
            _ => bail!("Channel creation when not in proposed state"),
        };
        Ok(())
    }

    pub fn proposal_result(&mut self, decision: bool) -> Result<(), Error> {
        *self = match self {
            ChannelManager::Proposed { accepted, state } => {
                if decision {
                    ChannelManager::PendingCreation {
                        state: state.clone(),
                        pending_send: 0.into(),
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
        if is_channel_acceptable(&their_prop.swap())? {
            *self = match self {
                ChannelManager::Proposed {
                    accepted: false,
                    state,
                } => {
                    let our_prop = state;
                    assert_ne!(their_prop.address_a, our_prop.address_a);
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
                ChannelManager::New => ChannelManager::PendingCreation {
                    state: their_prop.swap().clone(),
                    pending_send: 0.into(),
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
            state: CombinedState::new(&channel_a),
        };

        let m_b = ChannelManager::Open {
            state: CombinedState::new(&channel_b),
        };

        (m_a, m_b)
    }

    pub fn pay_counterparty(&mut self, amount: U256) -> Result<(), Error> {
        *self = match self {
            ChannelManager::Open { ref mut state } => {
                let overflow = state.pay_counterparty(amount)?;
                if overflow != 0.into() {
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
                if overflow != 0.into() {
                    // TODO: Handle reopening channel
                    bail!("not enough money to pay them")
                } else {
                    ChannelManager::Joined {
                        state: state.clone(),
                    }
                }
            }
            // TODO: Handle close and dispute
            _ => bail!("can only pay in open state"),
        };

        Ok(())
    }

    pub fn withdraw(&mut self) -> Result<U256, Error> {
        match self {
            ChannelManager::Open { ref mut state } | ChannelManager::Joined { ref mut state } => {
                Ok(state.withdraw()?)
            }
            // TODO: Handle close and dispute
            _ => Ok(0.into()),
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

    pub fn rec_payment(&mut self, payment: &UpdateTx) -> Result<UpdateTx, Error> {
        match self {
            ChannelManager::Open { ref mut state }
            | ChannelManager::Joined { ref mut state }
            | ChannelManager::PendingJoin { ref mut state, .. } => Ok(state.rec_payment(payment)?),
            // TODO: Handle close and dispute
            _ => bail!("we can only recieve payments in open or joined"),
        }
    }

    pub fn rec_updated_state(&mut self, rec_update: &UpdateTx) -> Result<(), Error> {
        match self {
            ChannelManager::Open { ref mut state }
            | ChannelManager::Joined { ref mut state }
            | ChannelManager::PendingJoin { ref mut state, .. } => {
                Ok(state.rec_updated_state(rec_update)?)
            }
            // TODO: Handle close and dispute
            _ => bail!("we can only recieve updated state in open or joined"),
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
    fn test_channel_opening() {
        let mut manager_a = ChannelManager::New;
        let mut manager_b = ChannelManager::New;

        let proposal = manager_a
            .propose_channel(1.into(), 2.into(), 100.into())
            .unwrap();

        let channel_prop = match proposal {
            ChannelManagerAction::SendChannelProposal(channel) => channel,
            _ => panic!("Wrong action returned"),
        };

        assert!(manager_b.check_proposal(&channel_prop).unwrap());
        manager_a.proposal_result(true).unwrap();

        let (channel_a, channel_b) = Channel::new_pair(100.into(), 0.into());

        assert_eq!(
            manager_a,
            ChannelManager::PendingCreation {
                state: channel_a.clone(),
                pending_send: 0.into(),
            }
        );

        assert_eq!(
            manager_b,
            ChannelManager::PendingCreation {
                state: channel_b.clone(),
                pending_send: 0.into(),
            }
        )
    }

    #[test]
    fn test_channel_opening_race() {
        let mut manager_a = ChannelManager::New;
        let mut manager_b = ChannelManager::New;

        let proposal_a = manager_a
            .propose_channel(1.into(), 2.into(), 100.into())
            .unwrap();

        let channel_prop_a = match proposal_a {
            ChannelManagerAction::SendChannelProposal(channel) => channel,
            _ => panic!("Wrong action returned"),
        };

        let proposal_b = manager_b
            .propose_channel(2.into(), 1.into(), 100.into())
            .unwrap();

        let channel_prop_b = match proposal_b {
            ChannelManagerAction::SendChannelProposal(channel) => channel,
            _ => panic!("Wrong action returned"),
        };

        assert!(manager_b.check_proposal(&channel_prop_a).unwrap());
        assert!(manager_a.check_proposal(&channel_prop_b).is_err());
        manager_a.proposal_result(true).unwrap();
        manager_b.proposal_result(false).unwrap();

        assert_eq!(
            manager_a,
            ChannelManager::PendingCreation {
                state: channel_prop_a.clone(),
                pending_send: 0.into(),
            }
        );

        manager_a
            .channel_created(&channel_prop_a, 1.into())
            .unwrap();
        manager_b
            .channel_created(&channel_prop_a, 2.into())
            .unwrap();

        assert_eq!(
            manager_a,
            ChannelManager::Joined {
                state: CombinedState::new(&channel_prop_a)
            }
        );

        assert_eq!(
            manager_b,
            ChannelManager::Open {
                state: CombinedState::new(&channel_prop_a.swap())
            }
        )
    }
}
