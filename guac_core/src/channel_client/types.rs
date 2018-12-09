use channel_client::combined_state::CombinedState;
use clarity::{Address, Signature};
use crypto::CryptoService;
use failure::Error;
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
