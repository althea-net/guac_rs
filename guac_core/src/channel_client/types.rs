use channel_client::combined_state::CombinedState;
use clarity::{Address, Signature};
use crypto::CryptoService;
use failure::{err_msg, Error};
use num256::Uint256;
use CRYPTO;

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub enum Counterparty {
    New {
        url: String,
        i_am_0: bool,
    },
    Creating {
        new_channel_tx: NewChannelTx,
        url: String,
        i_am_0: bool,
    },
    OtherCreating {
        new_channel_tx: NewChannelTx,
        url: String,
        i_am_0: bool,
    },
    ReDrawing {
        re_draw_tx: ReDrawTx,
        channel: CombinedState,
        url: String,
        // i_am_0: bool,
    },
    OtherReDrawing {
        re_draw_tx: ReDrawTx,
        channel: CombinedState,
        url: String,
        // i_am_0: bool,
    },
    Open {
        // last_update_tx:
        channel: CombinedState,
        url: String,
        // i_am_0: bool,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct NewChannelTx {
    pub address_0: Address,
    pub address_1: Address,

    pub balance_0: Uint256,
    pub balance_1: Uint256,

    pub expiration: Uint256,
    pub settling_period_length: Uint256,

    pub signature0: Option<Signature>,
    pub signature1: Option<Signature>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ReDrawTx {
    pub channel_id: Uint256,

    pub sequence_number: Uint256,
    pub old_balance_0: Uint256,
    pub old_balance_1: Uint256,

    pub new_balance_0: Uint256,
    pub new_balance_1: Uint256,

    pub expiration: Uint256,

    pub signature0: Option<Signature>,
    pub signature1: Option<Signature>,
}

impl NewChannelTx {
    pub fn sign(&self) -> Signature {
        unimplemented!();
    }
}

impl ReDrawTx {
    pub fn sign(&self) -> Signature {
        unimplemented!();
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Channel {
    pub channel_id: Uint256,
    pub address_0: Address,
    pub address_1: Address,

    pub total_balance: Uint256,
    pub balance_0: Uint256,
    pub balance_1: Uint256,
    pub sequence_number: Uint256,

    pub settling_period_length: Uint256,
    pub settling_period_started: bool,
    pub settling_period_end: Uint256,
    pub i_am_0: bool,
}

impl Channel {
    pub fn my_balance(&self) -> &Uint256 {
        match self.i_am_0 {
            true => &self.balance_0,
            false => &self.balance_1,
        }
    }
    pub fn their_balance(&self) -> &Uint256 {
        match self.i_am_0 {
            true => &self.balance_1,
            false => &self.balance_0,
        }
    }
    pub fn my_balance_mut(&mut self) -> &mut Uint256 {
        match self.i_am_0 {
            true => &mut self.balance_0,
            false => &mut self.balance_1,
        }
    }
    pub fn their_balance_mut(&mut self) -> &mut Uint256 {
        match self.i_am_0 {
            true => &mut self.balance_1,
            false => &mut self.balance_0,
        }
    }

    pub fn create_update(&self) -> UpdateTx {
        let mut update_tx = UpdateTx {
            channel_id: self.channel_id.clone(),
            sequence_number: self.sequence_number.clone(),
            balance_0: self.balance_0.clone(),
            balance_1: self.balance_1.clone(),
            signature_0: None,
            signature_1: None,
        };

        let signature = update_tx.sign();

        match self.i_am_0 {
            true => update_tx.signature_0 = Some(signature.clone()),
            false => update_tx.signature_1 = Some(signature.clone()),
        }

        update_tx
    }

    pub fn apply_update(&mut self, update: &UpdateTx, validate_balance: bool) -> Result<(), Error> {
        if update.channel_id != self.channel_id {
            bail!("update not for the right channel")
        }

        if !update.validate_their_signature(self.i_am_0) {
            bail!("sig is bad")
        }

        ensure!(
            update.their_balance(self.i_am_0).clone() + update.my_balance(self.i_am_0).clone()
                == self.total_balance,
            "balances do not add up to total balance"
        );

        ensure!(
            self.sequence_number < update.sequence_number,
            "Update too old"
        );

        if update.my_balance(self.i_am_0) < self.my_balance() && validate_balance {
            bail!("balance validation failed")
        }

        self.balance_0 = update.balance_0.clone();
        self.balance_1 = update.balance_1.clone();
        self.sequence_number = update.sequence_number.clone();

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UpdateTx {
    pub channel_id: Uint256,
    pub sequence_number: Uint256,

    pub balance_0: Uint256,
    pub balance_1: Uint256,

    pub signature_0: Option<Signature>,
    pub signature_1: Option<Signature>,
}

impl UpdateTx {
    pub fn sign(&mut self) -> Signature {
        let sequence_number: [u8; 32] = self.sequence_number.clone().into();
        let balance_0: [u8; 32] = self.balance_0.clone().into();
        let balance_1: [u8; 32] = self.balance_1.clone().into();

        let channel_id: [u8; 32] = self.channel_id.clone().into();

        let fingerprint =
            CRYPTO.hash_bytes(&[&channel_id, &sequence_number, &balance_0, &balance_1]);
        let fingerprint: [u8; 32] = fingerprint.clone().into();

        CRYPTO.eth_sign(&fingerprint)
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

// #[derive(Copy, Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
// pub enum ChannelStatus {
//     Open,
//     Joined,
//     Challenge,
//     Closed,
// }

// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
// pub struct OldChannel {
//     pub channel_id: Option<Uint256>,
//     pub address_a: Address,
//     pub address_b: Address,
//     pub channel_status: ChannelStatus,
//     pub deposit_a: Uint256,
//     pub deposit_b: Uint256,
//     pub challenge: Uint256,
//     pub nonce: Uint256,
//     pub close_time: Uint256,
//     pub balance_a: Uint256,
//     pub balance_b: Uint256,
//     pub is_a: bool,
// }

// impl OldChannel {
//     pub fn new_pair(
//         channel_id: Uint256,
//         deposit_a: Uint256,
//         deposit_b: Uint256,
//     ) -> (OldChannel, OldChannel) {
//         let channel_a = OldChannel {
//             channel_id: Some(channel_id.clone()),
//             address_a: "0x0000000000000000000000000000000000000001"
//                 .parse()
//                 .unwrap(),
//             address_b: "0x0000000000000000000000000000000000000002"
//                 .parse()
//                 .unwrap(),
//             channel_status: ChannelStatus::Joined,
//             deposit_a: deposit_a.clone(),
//             deposit_b: deposit_b.clone(),
//             challenge: 0u64.into(),
//             nonce: 0u64.into(),
//             close_time: 10u64.into(),
//             balance_a: deposit_a,
//             balance_b: deposit_b,
//             is_a: true,
//         };

//         let channel_b = OldChannel {
//             is_a: false,
//             ..channel_a.clone()
//         };

//         (channel_a, channel_b)
//     }

//     pub fn total_deposit(&self) -> Uint256 {
//         self.deposit_a.clone() + self.deposit_b.clone()
//     }

//     pub fn swap(&self) -> Self {
//         OldChannel {
//             is_a: !self.is_a,
//             ..self.clone()
//         }
//     }
// }

// impl OldChannel {
//     pub fn get_my_address(&self) -> &Address {
//         match self.is_a {
//             true => &self.address_a,
//             false => &self.address_b,
//         }
//     }
//     pub fn get_their_address(&self) -> &Address {
//         match self.is_a {
//             true => &self.address_b,
//             false => &self.address_a,
//         }
//     }
//     pub fn my_balance(&self) -> &Uint256 {
//         match self.is_a {
//             true => &self.balance_a,
//             false => &self.balance_b,
//         }
//     }
//     pub fn their_balance(&self) -> &Uint256 {
//         match self.is_a {
//             true => &self.balance_b,
//             false => &self.balance_a,
//         }
//     }
//     pub fn my_balance_mut(&mut self) -> &mut Uint256 {
//         match self.is_a {
//             true => &mut self.balance_a,
//             false => &mut self.balance_b,
//         }
//     }
//     pub fn their_balance_mut(&mut self) -> &mut Uint256 {
//         match self.is_a {
//             true => &mut self.balance_b,
//             false => &mut self.balance_a,
//         }
//     }
//     pub fn my_deposit(&self) -> &Uint256 {
//         match self.is_a {
//             true => &self.deposit_a,
//             false => &self.deposit_b,
//         }
//     }
//     pub fn their_deposit(&self) -> &Uint256 {
//         match self.is_a {
//             true => &self.deposit_b,
//             false => &self.deposit_a,
//         }
//     }
//     pub fn my_deposit_mut(&mut self) -> &mut Uint256 {
//         match self.is_a {
//             true => &mut self.deposit_a,
//             false => &mut self.deposit_b,
//         }
//     }
//     pub fn their_deposit_mut(&mut self) -> &mut Uint256 {
//         match self.is_a {
//             true => &mut self.deposit_b,
//             false => &mut self.deposit_a,
//         }
//     }
//     pub fn create_update(&self) -> Result<OldUpdateTx, Error> {
//         let channel_id = self.channel_id.as_ref().ok_or(err_msg(
//             "Unable to create update before channel is open on the network",
//         ))?;

//         let mut update_tx = OldUpdateTx {
//             channel_id: channel_id.clone(),
//             nonce: self.nonce.clone(),
//             balance_a: self.balance_a.clone(),
//             balance_b: self.balance_b.clone(),
//             signature_a: None,
//             signature_b: None,
//         };

//         update_tx.sign(self.is_a, channel_id.clone());
//         Ok(update_tx)
//     }
//     pub fn apply_update(
//         &mut self,
//         update: &OldUpdateTx,
//         validate_balance: bool,
//     ) -> Result<(), Error> {
//         trace!(
//             "Apply update for channel {:?} with {:?}",
//             self.channel_id,
//             update.channel_id
//         );
//         ensure!(
//             self.channel_id.is_some(),
//             "Unable to apply update before opening a channel on the network"
//         );
//         if update.channel_id != *self.channel_id.as_ref().unwrap() {
//             bail!("update not for the right channel")
//         }

//         if !update.validate_their_signature(self.is_a) {
//             bail!("sig is bad")
//         }

//         ensure!(
//             update.their_balance(self.is_a).clone() + update.my_balance(self.is_a).clone()
//                 == self.my_balance().clone() + self.their_balance().clone(),
//             "balance does not add up"
//         );

//         ensure!(
//             update.their_balance(self.is_a).clone() + update.my_balance(self.is_a).clone()
//                 == self.deposit_a.clone() + self.deposit_b.clone(),
//             "balance does not add up to deposit values"
//         );

//         if self.nonce > update.nonce {
//             bail!("Update too old");
//         }

//         if update.my_balance(self.is_a) < self.my_balance() && validate_balance {
//             bail!("balance validation failed")
//         }

//         self.balance_a = update.balance_a.clone();
//         self.balance_b = update.balance_b.clone();
//         self.nonce = update.nonce.clone();

//         Ok(())
//     }
// }

// #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
// pub struct OldUpdateTx {
//     pub channel_id: Uint256,
//     pub nonce: Uint256,

//     pub balance_a: Uint256,
//     pub balance_b: Uint256,

//     pub signature_a: Option<Signature>,
//     pub signature_b: Option<Signature>,
// }

// impl OldUpdateTx {
//     pub fn set_my_signature(&mut self, is_a: bool, signature: &Signature) {
//         match is_a {
//             true => self.signature_a = Some(signature.clone()),
//             false => self.signature_b = Some(signature.clone()),
//         }
//     }
//     pub fn validate_their_signature(&self, _is_a: bool) -> bool {
//         // TODO: actually do validation
//         true
//     }
//     pub fn their_balance(&self, is_a: bool) -> &Uint256 {
//         match is_a {
//             true => &self.balance_b,
//             false => &self.balance_a,
//         }
//     }
//     pub fn my_balance(&self, is_a: bool) -> &Uint256 {
//         match is_a {
//             true => &self.balance_a,
//             false => &self.balance_b,
//         }
//     }
//     pub fn set_their_signature(&mut self, is_a: bool, signature: &Signature) {
//         match is_a {
//             true => self.signature_b = Some(signature.clone()),
//             false => self.signature_a = Some(signature.clone()),
//         }
//     }

//     pub fn sign(&mut self, is_a: bool, channel_id: Uint256) {
//         let nonce: [u8; 32] = self.nonce.clone().into();
//         let balance_a: [u8; 32] = self.balance_a.clone().into();
//         let balance_b: [u8; 32] = self.balance_b.clone().into();

//         let channel_id: [u8; 32] = channel_id.clone().into();

//         let fingerprint = CRYPTO.hash_bytes(&[&channel_id, &nonce, &balance_a, &balance_b]);
//         let fingerprint: [u8; 32] = fingerprint.clone().into();

//         let my_sig = CRYPTO.eth_sign(&fingerprint);

//         self.set_my_signature(is_a, &my_sig.into());
//     }

//     pub fn strip_sigs(&self) -> OldUpdateTx {
//         OldUpdateTx {
//             signature_a: None,
//             signature_b: None,
//             ..self.clone()
//         }
//     }
// }
