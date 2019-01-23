use crate::channel::Channel;
use crate::crypto;
use clarity::{Address, Signature};
use num256::Uint256;
use std::vec::Vec;

#[derive(Debug, Fail)]
pub enum GuacError {
    #[fail(
        display = "Guac is currently waiting on another operation to complete. Try again later."
    )]
    TryAgainLater(),

    #[fail(
        display = "Cannot {} in the current state: {}. State must be: {}",
        action, current_state, correct_state
    )]
    WrongState {
        action: String,
        current_state: String,
        correct_state: String,
    },

    #[fail(display = "Invalid request: {}", message)]
    Forbidden { message: String },

    #[fail(display = "Update too old. Correct sequence number: {}", correct_seq)]
    UpdateTooOld { correct_seq: Uint256 },

    #[fail(display = "Not enough {}", stuff)]
    NotEnough { stuff: String },

    #[fail(display = "Something has gone wrong: {}", message)]
    Error { message: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Counterparty {
    New {
        i_am_0: bool,
    },
    Creating {
        new_channel_tx: NewChannelTx,
        i_am_0: bool,
    },
    OtherCreating {
        new_channel_tx: NewChannelTx,
        i_am_0: bool,
    },
    ReDrawing {
        re_draw_tx: ReDrawTx,
        channel: Channel,
    },
    OtherReDrawing {
        re_draw_tx: ReDrawTx,
        channel: Channel,
    },
    Open {
        channel: Channel,
    },
}

// TODO: Figure out real solidity bytes representation of bool instead of just guessing
fn bool_to_bytes(boo: bool) -> [u8; 32] {
    if boo {
        let res: Uint256 = 1u8.into();
        let res: [u8; 32] = res.into();
        res
    } else {
        let res: Uint256 = 0u8.into();
        let res: [u8; 32] = res.into();
        res
    }
}

impl Channel {
    pub fn fingerprint(&self, contract_address: Address) -> [u8; 32] {
        let func_name: &[u8] = "Channel".as_bytes();
        let contract_address: &[u8] = contract_address.as_bytes();
        let channel_id: [u8; 32] = self.channel_id.clone().into();
        let sequence_number: [u8; 32] = self.sequence_number.clone().into();
        let balance_0: [u8; 32] = self.balance_0.clone().into();
        let balance_1: [u8; 32] = self.balance_1.clone().into();
        let accrual: [u8; 32] = self.accrual.clone().into();
        let i_am_0: [u8; 32] = bool_to_bytes(self.i_am_0);

        let fingerprint = crypto::hash_bytes(&[
            func_name,
            contract_address,
            &channel_id,
            &sequence_number,
            &balance_0,
            &balance_1,
            &accrual,
            &i_am_0,
        ]);
        let fingerprint: [u8; 32] = fingerprint.clone().into();

        return fingerprint;
    }
}

impl Counterparty {
    pub fn fingerprint(&self, contract_address: Address) -> [u8; 32] {
        match self {
            Counterparty::New { i_am_0 } => {
                let name: &[u8] = "Counterparty".as_bytes();
                let caddr: &[u8] = contract_address.as_bytes();
                let variant: &[u8] = "New".as_bytes();
                let i_am_0: [u8; 32] = bool_to_bytes(*i_am_0);

                let fingerprint = crypto::hash_bytes(&[name, caddr, variant, &i_am_0]);
                let fingerprint: [u8; 32] = fingerprint.clone().into();

                return fingerprint;
            }
            Counterparty::Creating {
                new_channel_tx,
                i_am_0,
            } => {
                let name: &[u8] = "Counterparty".as_bytes();
                let caddr: &[u8] = contract_address.as_bytes();
                let variant: &[u8] = "Creating".as_bytes();
                let new_channel_tx: &[u8] = &new_channel_tx.fingerprint(contract_address);
                let i_am_0: [u8; 32] = bool_to_bytes(*i_am_0);

                let fingerprint =
                    crypto::hash_bytes(&[name, caddr, variant, new_channel_tx, &i_am_0]);
                let fingerprint: [u8; 32] = fingerprint.clone().into();

                return fingerprint;
            }
            Counterparty::OtherCreating {
                new_channel_tx,
                i_am_0,
            } => {
                let name: &[u8] = "Counterparty".as_bytes();
                let caddr: &[u8] = contract_address.as_bytes();
                let variant: &[u8] = "OtherCreating".as_bytes();
                let new_channel_tx: &[u8] = &new_channel_tx.fingerprint(contract_address);
                let i_am_0: [u8; 32] = bool_to_bytes(*i_am_0);

                let fingerprint =
                    crypto::hash_bytes(&[name, caddr, variant, new_channel_tx, &i_am_0]);
                let fingerprint: [u8; 32] = fingerprint.clone().into();

                return fingerprint;
            }
            Counterparty::ReDrawing {
                re_draw_tx,
                channel,
            } => {
                let name: &[u8] = "Counterparty".as_bytes();
                let caddr: &[u8] = contract_address.as_bytes();
                let variant: &[u8] = "ReDrawing".as_bytes();
                let re_draw_tx: &[u8] = &re_draw_tx.fingerprint(contract_address);
                let channel: &[u8] = &channel.fingerprint(contract_address);

                let fingerprint = crypto::hash_bytes(&[name, caddr, variant, re_draw_tx, channel]);
                let fingerprint: [u8; 32] = fingerprint.clone().into();

                return fingerprint;
            }
            Counterparty::OtherReDrawing {
                re_draw_tx,
                channel,
            } => {
                let name: &[u8] = "Counterparty".as_bytes();
                let caddr: &[u8] = contract_address.as_bytes();
                let variant: &[u8] = "OtherReDrawing".as_bytes();
                let re_draw_tx: &[u8] = &re_draw_tx.fingerprint(contract_address);
                let channel: &[u8] = &channel.fingerprint(contract_address);

                let fingerprint = crypto::hash_bytes(&[name, caddr, variant, re_draw_tx, channel]);
                let fingerprint: [u8; 32] = fingerprint.clone().into();

                return fingerprint;
            }
            Counterparty::Open { channel } => {
                let name: &[u8] = "Counterparty".as_bytes();
                let caddr: &[u8] = contract_address.as_bytes();
                let variant: &[u8] = "Open".as_bytes();
                let channel: &[u8] = &channel.fingerprint(contract_address);

                let fingerprint = crypto::hash_bytes(&[name, caddr, variant, channel]);
                let fingerprint: [u8; 32] = fingerprint.clone().into();

                return fingerprint;
            }
        }
    }
}

pub struct CounterpartySave {
    counterparty: Counterparty,
    signature: Signature,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct NewChannelTx {
    pub address_0: Address,
    pub address_1: Address,

    pub balance_0: Uint256,
    pub balance_1: Uint256,

    pub expiration: Uint256,
    pub settling_period_length: Uint256,

    pub signature_0: Option<Signature>,
    pub signature_1: Option<Signature>,
}

impl NewChannelTx {
    pub fn fingerprint(&self, contract_address: Address) -> [u8; 32] {
        let func_name: &[u8] = "newChannel".as_bytes();
        let contract_address: &[u8] = contract_address.as_bytes();
        let address_0: &[u8] = self.address_0.as_bytes();
        let address_1: &[u8] = self.address_1.as_bytes();
        let balance_0: [u8; 32] = self.balance_0.clone().into();
        let balance_1: [u8; 32] = self.balance_1.clone().into();
        let expiration: [u8; 32] = self.expiration.clone().into();
        let settling_period_length: [u8; 32] = self.settling_period_length.clone().into();

        let fingerprint = crypto::hash_bytes(&[
            func_name,
            contract_address,
            &address_0,
            &address_1,
            &balance_0,
            &balance_1,
            &expiration,
            &settling_period_length,
        ]);
        let fingerprint: [u8; 32] = fingerprint.clone().into();

        return fingerprint;
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ReDrawTx {
    pub channel_id: [u8; 32],

    pub sequence_number: Uint256,
    pub old_balance_0: Uint256,
    pub old_balance_1: Uint256,

    pub new_balance_0: Uint256,
    pub new_balance_1: Uint256,

    pub expiration: Uint256,

    pub signature_0: Option<Signature>,
    pub signature_1: Option<Signature>,
}

impl ReDrawTx {
    pub fn fingerprint(&self, contract_address: Address) -> [u8; 32] {
        let func_name: &[u8] = "reDraw".as_bytes();
        let contract_address: &[u8] = contract_address.as_bytes();
        let channel_id: [u8; 32] = self.channel_id.clone().into();
        let sequence_number: [u8; 32] = self.sequence_number.clone().into();
        let old_balance_0: [u8; 32] = self.old_balance_0.clone().into();
        let old_balance_1: [u8; 32] = self.old_balance_1.clone().into();
        let new_balance_0: [u8; 32] = self.new_balance_0.clone().into();
        let new_balance_1: [u8; 32] = self.new_balance_1.clone().into();
        let expiration: [u8; 32] = self.expiration.clone().into();

        let fingerprint = crypto::hash_bytes(&[
            func_name,
            contract_address,
            &channel_id,
            &sequence_number,
            &old_balance_0,
            &old_balance_1,
            &new_balance_0,
            &new_balance_1,
            &expiration,
        ]);
        let fingerprint: [u8; 32] = fingerprint.clone().into();

        return fingerprint;
    }
}

// #[derive(Clone, Debug, Serialize, PartialEq, Eq)]
// pub struct Channel {
//     pub channel_id: [u8; 32],
//     pub address_0: Address,
//     pub address_1: Address,

//     pub total_balance: Uint256,
//     pub balance_0: Uint256,
//     pub balance_1: Uint256,
//     pub sequence_number: Uint256,

//     pub settling_period_length: Uint256,
//     pub settling_period_started: bool,
//     pub settling_period_end: Uint256,
//     pub i_am_0: bool,
// }

// impl Channel {
//     pub fn my_balance(&self) -> &Uint256 {
//         match self.i_am_0 {
//             true => &self.balance_0,
//             false => &self.balance_1,
//         }
//     }
//     pub fn their_balance(&self) -> &Uint256 {
//         match self.i_am_0 {
//             true => &self.balance_1,
//             false => &self.balance_0,
//         }
//     }
//     pub fn my_balance_mut(&mut self) -> &mut Uint256 {
//         match self.i_am_0 {
//             true => &mut self.balance_0,
//             false => &mut self.balance_1,
//         }
//     }
//     pub fn their_balance_mut(&mut self) -> &mut Uint256 {
//         match self.i_am_0 {
//             true => &mut self.balance_1,
//             false => &mut self.balance_0,
//         }
//     }

//     pub fn create_update(&self) -> UpdateTx {
//         UpdateTx {
//             channel_id: self.channel_id.clone(),
//             sequence_number: self.sequence_number.clone(),
//             balance_0: self.balance_0.clone(),
//             balance_1: self.balance_1.clone(),
//             signature_0: None,
//             signature_1: None,
//         }
//     }

//     pub fn apply_update(&mut self, update: &UpdateTx, validate_balance: bool) -> Result<(), Error> {
//         if update.channel_id != self.channel_id {
//             bail!("update not for the right channel")
//         }

//         if !update.validate_their_signature(self.i_am_0) {
//             bail!("sig is bad")
//         }

//         ensure!(
//             update.their_balance(self.i_am_0).clone() + update.my_balance(self.i_am_0).clone()
//                 == self.total_balance,
//             "balances do not add up to total balance"
//         );

//         if self.sequence_number < update.sequence_number {
//             return Err(GuacError::UpdateTooOld().into());
//         }

//         if update.my_balance(self.i_am_0) < self.my_balance() && validate_balance {
//             bail!("balance validation failed")
//         }

//         self.balance_0 = update.balance_0.clone();
//         self.balance_1 = update.balance_1.clone();
//         self.sequence_number = update.sequence_number.clone();

//         Ok(())
//     }
// }

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UpdateTx {
    pub channel_id: [u8; 32],
    pub sequence_number: Uint256,

    pub balance_0: Uint256,
    pub balance_1: Uint256,

    pub signature_0: Option<Signature>,
    pub signature_1: Option<Signature>,
}

impl UpdateTx {
    pub fn fingerprint(&self, contract_address: Address) -> [u8; 32] {
        let func_name: &[u8] = "Update".as_bytes();
        let contract_address: &[u8] = contract_address.as_bytes();
        let channel_id: [u8; 32] = self.channel_id.clone().into();
        let sequence_number: [u8; 32] = self.sequence_number.clone().into();
        let balance_0: [u8; 32] = self.balance_0.clone().into();
        let balance_1: [u8; 32] = self.balance_1.clone().into();

        let fingerprint = crypto::hash_bytes(&[
            func_name,
            contract_address,
            &channel_id,
            &sequence_number,
            &balance_0,
            &balance_1,
        ]);
        let fingerprint: [u8; 32] = fingerprint.clone().into();

        return fingerprint;
    }

    pub fn set_my_signature(&mut self, i_am_0: bool, signature: &Signature) {
        match i_am_0 {
            true => self.signature_0 = Some(signature.clone()),
            false => self.signature_1 = Some(signature.clone()),
        }
    }
    pub fn validate_their_signature(&self, _i_am_0: bool) -> bool {
        // TODO: actually do validation
        true
    }
    pub fn their_balance(&self, i_am_0: bool) -> &Uint256 {
        match i_am_0 {
            true => &self.balance_1,
            false => &self.balance_0,
        }
    }
    pub fn my_balance(&self, i_am_0: bool) -> &Uint256 {
        match i_am_0 {
            true => &self.balance_0,
            false => &self.balance_1,
        }
    }
}
