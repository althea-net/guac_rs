use crate::channel_client::combined_state::CombinedState;
use crate::channel_client::types::UpdateTx;
use crate::channel_client::types::{Counterparty, NewChannelTx, ReDrawTx};
use crate::channel_client::Channel;
use clarity::{Address, PrivateKey, Signature};

use failure::Error;
use futures::{future, Future};
use crate::new_crypto;
use crate::new_crypto::Crypto;
use num256::Uint256;

use std::sync::Arc;
use crate::storage::Storage;

#[macro_export]
macro_rules! try_future_box {
    ($expression:expr) => {
        match $expression {
            Err(err) => {
                return Box::new(future::err(err.into())) as Box<Future<Item = _, Error = Error>>;
            }
            Ok(value) => value,
        }
    };
}

#[derive(Clone)]
pub struct Guac {
    pub blockchain_client: Arc<Box<BlockchainApi + Send + Sync>>,
    pub counterparty_client: Arc<Box<CounterpartyApi + Send + Sync>>,
    pub storage: Arc<Box<Storage>>,
    pub crypto: Arc<Box<Crypto>>,
}

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
}

pub trait BlockchainApi {
    fn new_channel(
        &self,
        new_channel_tx: NewChannelTx,
    ) -> Box<Future<Item = [u8; 32], Error = Error>>;

    fn re_draw(&self, redraw_tx: ReDrawTx) -> Box<Future<Item = (), Error = Error>>;

    fn check_for_open(
        &self,
        address_0: &Address,
        address_1: &Address,
    ) -> Box<Future<Item = [u8; 32], Error = Error>>;

    fn check_for_re_draw(&self, channel_id: [u8; 32]) -> Box<Future<Item = (), Error = Error>>;

    fn quick_deposit(&self, value: Uint256) -> Box<Future<Item = (), Error = Error>>;
}

pub trait UserApi {
    fn register_counterparty(
        &self,
        their_address: Address,
        url: String,
    ) -> Box<Future<Item = (), Error = Error>>;
    fn fill_channel(
        &self,
        their_address: Address,
        amount: Uint256,
    ) -> Box<Future<Item = (), Error = Error>>;

    fn make_payment(
        &self,
        their_address: Address,
        amount: Uint256,
    ) -> Box<Future<Item = (), Error = Error>>;
}

pub trait CounterpartyApi {
    fn propose_channel(
        &self,
        from_address: Address,
        to_url: String,
        new_channel_tx: NewChannelTx,
    ) -> Box<Future<Item = Signature, Error = Error>>;

    fn propose_re_draw(
        &self,
        from_address: Address,
        to_url: String,
        re_draw_tx: ReDrawTx,
    ) -> Box<Future<Item = Signature, Error = Error>>;

    fn notify_channel_opened(
        &self,
        from_address: Address,
        to_url: String,
    ) -> Box<Future<Item = (), Error = Error>>;

    fn notify_re_draw(
        &self,
        from_address: Address,
        to_url: String,
    ) -> Box<Future<Item = (), Error = Error>>;

    fn receive_payment(
        &self,
        from_address: Address,
        to_url: String,
        update_tx: UpdateTx,
    ) -> Box<Future<Item = UpdateTx, Error = Error>>;
}

impl UserApi for Guac {
    fn register_counterparty(
        &self,
        their_address: Address,
        url: String,
    ) -> Box<Future<Item = (), Error = Error>> {
        let storage = self.storage.clone();
        let crypto = self.crypto.clone();

        Box::new(storage.new_counterparty(
            their_address.clone(),
            Counterparty::New {
                url,
                i_am_0: crypto.own_address < their_address,
            },
        ))
    }

    fn fill_channel(
        &self,
        their_address: Address,
        amount: Uint256,
    ) -> Box<Future<Item = (), Error = Error>> {
        let counterparty_client = self.counterparty_client.clone();
        let blockchain_client = self.blockchain_client.clone();
        let storage = self.storage.clone();
        let crypto = self.crypto.clone();

        Box::new(storage.get_counterparty(their_address.clone()).and_then(
            move |mut counterparty| {
                match counterparty.clone() {
                    Counterparty::New { i_am_0, url } => {
                        let my_address = crypto.own_address;

                        let (address_0, address_1) = if i_am_0 {
                            (my_address, their_address)
                        } else {
                            (their_address, my_address)
                        };

                        let (balance_0, balance_1) = if i_am_0 {
                            (amount, 0u64.into())
                        } else {
                            (0u64.into(), amount)
                        };

                        let new_channel_tx = NewChannelTx {
                            address_0: address_0.clone(),
                            address_1: address_1.clone(),
                            balance_0: balance_0.clone(),
                            balance_1: balance_1.clone(),
                            expiration: 9999999999u64.into(), //TODO: get current block plus some
                            settling_period_length: 5000u64.into(), //TODO: figure out default value
                            signature_0: None,
                            signature_1: None,
                        };

                        let my_signature = crypto
                            .eth_sign(&new_channel_tx.clone().fingerprint(crypto.contract_address));

                        Box::new(
                            counterparty_client
                                .propose_channel(my_address, url.clone(), new_channel_tx.clone())
                                .and_then(move |their_signature| {
                                    let (signature_0, signature_1) = if i_am_0 {
                                        (my_signature, their_signature)
                                    } else {
                                        (their_signature, my_signature)
                                    };

                                    *counterparty = Counterparty::Creating {
                                        new_channel_tx: new_channel_tx.clone(),
                                        i_am_0,
                                        url: url.clone(),
                                    };

                                    blockchain_client
                                        .new_channel(NewChannelTx {
                                            signature_0: Some(signature_0),
                                            signature_1: Some(signature_1),
                                            ..new_channel_tx
                                        })
                                        .and_then(move |channel_id| {
                                            counterparty_client
                                                .notify_channel_opened(my_address, url.clone())
                                                .and_then(move |()| {
                                                    *counterparty = Counterparty::Open {
                                                        channel: CombinedState::new(&Channel {
                                                            channel_id,
                                                            address_0,
                                                            address_1,
                                                            total_balance: balance_0.clone()
                                                                + balance_1.clone(),
                                                            balance_0,
                                                            balance_1,
                                                            sequence_number: 0u64.into(),
                                                            settling_period_length: 5000u64.into(),
                                                            settling_period_started: false,
                                                            settling_period_end: 0u64.into(),
                                                            i_am_0,
                                                        }),
                                                        url: url.clone(),
                                                    };
                                                    Ok(())
                                                })
                                        })
                                }),
                        ) as Box<Future<Item = (), Error = Error>>
                    }
                    Counterparty::Open { channel, url } => {
                        let url = url.clone();

                        let balance_0 = channel.my_state.balance_0.clone();
                        let balance_1 = channel.my_state.balance_1.clone();

                        let (new_balance_0, new_balance_1) = if channel.my_state.i_am_0 {
                            (balance_0 + amount, balance_1)
                        } else {
                            (balance_0, balance_1 + amount)
                        };

                        let re_draw_tx = ReDrawTx {
                            channel_id: channel.my_state.channel_id.clone(),
                            sequence_number: channel.my_state.sequence_number.clone(),
                            old_balance_0: channel.my_state.balance_0.clone(),
                            old_balance_1: channel.my_state.balance_1.clone(),
                            new_balance_0: new_balance_0.clone(),
                            new_balance_1: new_balance_1.clone(),
                            expiration: 9999999999u64.into(), //TODO: get current block plus some,
                            signature_0: None,
                            signature_1: None,
                        };

                        let my_signature =
                            crypto.eth_sign(&re_draw_tx.fingerprint(crypto.contract_address));

                        Box::new(
                            counterparty_client
                                .propose_re_draw(
                                    crypto.own_address,
                                    url.clone(),
                                    re_draw_tx.clone(),
                                )
                                .and_then(move |their_signature| {
                                    *counterparty = Counterparty::ReDrawing {
                                        channel: channel.clone(),
                                        re_draw_tx: re_draw_tx.clone(),
                                        url: url.clone(),
                                    };
                                    let (signature_0, signature_1) =
                                        if channel.clone().my_state.i_am_0 {
                                            (my_signature, their_signature)
                                        } else {
                                            (their_signature, my_signature)
                                        };

                                    blockchain_client
                                        .re_draw(ReDrawTx {
                                            signature_0: Some(signature_0),
                                            signature_1: Some(signature_1),
                                            ..re_draw_tx
                                        })
                                        .and_then(move |_| {
                                            counterparty_client
                                                .notify_re_draw(crypto.own_address, url.clone())
                                                .and_then(move |_| {
                                                    // Save the new open state of the channel
                                                    *counterparty = Counterparty::Open {
                                                        url: url.clone(),
                                                        channel: CombinedState::new(&Channel {
                                                            // TODO: what else changes here?
                                                            balance_0: new_balance_0.clone(),
                                                            balance_1: new_balance_1.clone(),
                                                            ..channel.my_state
                                                        }),
                                                    };
                                                    Ok(())
                                                })
                                        })
                                }),
                        ) as Box<Future<Item = (), Error = Error>>
                    }
                    _ => {
                        // Make user wait
                        return Box::new(future::err(GuacError::TryAgainLater().into()))
                            as Box<Future<Item = (), Error = Error>>;
                    }
                }
            },
        ))
    }

    fn make_payment(
        &self,
        their_address: Address,
        amount: Uint256,
    ) -> Box<Future<Item = (), Error = Error>> {
        let storage = self.storage.clone();
        let counterparty_client = self.counterparty_client.clone();
        let crypto = self.crypto.clone();

        Box::new(storage.get_counterparty(their_address.clone()).and_then(
            move |mut counterparty| {
                match counterparty.clone() {
                    Counterparty::Open { mut channel, url } => {
                        try_future_box!(channel.make_payment(amount));
                        let update_tx = try_future_box!(channel.create_update());

                        Box::new(
                            counterparty_client
                                .receive_payment(crypto.own_address, url.clone(), update_tx.clone())
                                .and_then(move |their_update_tx| {
                                    channel.receive_payment_ack(&their_update_tx)?;

                                    *counterparty = Counterparty::Open { channel, url };
                                    Ok(())
                                }),
                        ) as Box<Future<Item = (), Error = Error>>
                    }
                    _ => {
                        let error = GuacError::WrongState {
                            correct_state: "Open".to_string(),
                            current_state: format!("{:?}", counterparty.clone()),
                            action: "make payment".to_string(),
                        };
                        return Box::new(future::err(error.into())) // TODO: Design a better set of errors, and when to use them
                                as Box<Future<Item = (), Error = Error>>;
                    }
                }
            },
        ))
    }
}

impl CounterpartyApi for Guac {
    fn propose_channel(
        &self,
        from_address: Address,
        _to_url: String,
        new_channel_tx: NewChannelTx,
    ) -> Box<Future<Item = Signature, Error = Error>> {
        let storage = self.storage.clone();
        let crypto = self.crypto.clone();
        let new_channel_tx_clone_1 = new_channel_tx.clone();
        let new_channel_tx_clone_2 = new_channel_tx.clone();
        let my_address = crypto.own_address;

        Box::new(
            storage
                .get_counterparty(from_address.clone())
                .and_then(move |mut counterparty| {
                    match counterparty.clone() {
                        Counterparty::New { url, i_am_0 } => {
                            Box::new(
                                future::ok(())
                                    .and_then(move |_| {
                                        let NewChannelTx {
                                            address_0,
                                            address_1,
                                            balance_0,
                                            balance_1,
                                            expiration: _,
                                            settling_period_length,
                                            signature_0: _,
                                            signature_1: _,
                                        } = new_channel_tx_clone_1;

                                        ensure!(address_0 < address_1, "Addresses must be sorted.");

                                        let my_balance = if address_0 == my_address {
                                            (balance_0)
                                        } else if address_1 == my_address {
                                            (balance_1)
                                        } else {
                                            bail!("This is NewChannelTx is not meant for me.")
                                        };

                                        ensure!(
                                            my_balance == 0u64.into(),
                                            "My balance in proposed channel must be zero."
                                        );

                                        ensure!(
                                            settling_period_length == 5000u64.into(),
                                            "I only accept settling periods of 5000 blocks"
                                        );
                                        Ok(())
                                    })
                                    .and_then(move |_| {
                                        // Save the current state of the counterparty
                                        *counterparty = Counterparty::OtherCreating {
                                            i_am_0,
                                            new_channel_tx: new_channel_tx_clone_2.clone(),
                                            url,
                                        };

                                        let my_signature = crypto.eth_sign(
                                            &new_channel_tx_clone_2
                                                .fingerprint(crypto.contract_address),
                                        );
                                        Ok(my_signature)
                                    }),
                            )
                                as Box<Future<Item = Signature, Error = Error>>
                        }
                        _ => {
                            let error = GuacError::WrongState {
                                correct_state: "New".to_string(),
                                current_state: format!("{:?}", counterparty.clone()),
                                action: "propose channel".to_string(),
                            };
                            return Box::new(future::err(error.into())) // TODO: Design a better set of errors, and when to use them
                                as Box<Future<Item = Signature, Error = Error>>;
                        }
                    }
                }),
        )
    }

    fn propose_re_draw(
        &self,
        from_address: Address,
        _to_url: String,
        re_draw_tx: ReDrawTx,
    ) -> Box<Future<Item = Signature, Error = Error>> {
        let storage = self.storage.clone();
        let crypto = self.crypto.clone();

        Box::new(
            storage
                .get_counterparty(from_address.clone())
                .and_then(move |mut counterparty| {
                    match counterparty.clone() {
                        Counterparty::Open { url, channel } => {
                            let channel_clone_1 = channel.clone();
                            let re_draw_tx_clone_1 = re_draw_tx.clone();
                            Box::new(
                                // Have to do this weird thing with future::ok to get ensure! to work
                                future::ok(()).and_then(move |_| {
                                    let ReDrawTx {
                                        channel_id,

                                        sequence_number,
                                        old_balance_0,
                                        old_balance_1,

                                        new_balance_0,
                                        new_balance_1,

                                        expiration: _,

                                        signature_0: _,
                                        signature_1: _,
                                    } = re_draw_tx;

                                    ensure!(
                                        channel_id == channel.my_state.channel_id,
                                        "Incorrect channel ID."
                                    );
                                    ensure!(
                                        sequence_number == channel.my_state.sequence_number,
                                        "Incorrect sequence number."
                                    );
                                    ensure!(
                                        old_balance_0 == channel.my_state.balance_0,
                                        "Incorrect old balance_0"
                                    );
                                    ensure!(
                                        old_balance_1 == channel.my_state.balance_1,
                                        "Incorrect old balance_1"
                                    );

                                    if channel.my_state.i_am_0 {
                                        ensure!(
                                            new_balance_0 == channel.my_state.balance_0,
                                            "Incorrect new balance_0"
                                        );
                                    } else {
                                        ensure!(
                                            new_balance_1 == channel.my_state.balance_1,
                                            "Incorrect new balance_1"
                                        );
                                    }

                                    *counterparty = Counterparty::OtherReDrawing {
                                        channel: channel_clone_1,
                                        re_draw_tx: re_draw_tx_clone_1.clone(),
                                        url,
                                    };

                                    let my_signature = crypto.eth_sign(
                                        &re_draw_tx_clone_1.fingerprint(crypto.contract_address),
                                    );

                                    Ok(my_signature)
                                }),
                            )
                                as Box<Future<Item = Signature, Error = Error>>
                        }
                        _ => {
                            let error = GuacError::WrongState {
                                correct_state: "Open".to_string(),
                                current_state: format!("{:?}", counterparty.clone()),
                                action: "propose redraw".to_string(),
                            };
                            return Box::new(future::err(error.into())) // TODO: Design a better set of errors, and when to use them
                                as Box<Future<Item = Signature, Error = Error>>;
                        }
                    }
                }),
        )
    }

    fn notify_channel_opened(
        &self,
        from_address: Address,
        _to_url: String,
    ) -> Box<Future<Item = (), Error = Error>> {
        let storage = self.storage.clone();
        let blockchain_client = self.blockchain_client.clone();
        let crypto = self.crypto.clone();
        Box::new(
            storage
                .get_counterparty(from_address.clone())
                .and_then(move |mut counterparty| {
                    match counterparty.clone() {
                        Counterparty::OtherCreating {
                            i_am_0,
                            url,
                            new_channel_tx,
                        } => {
                            let (address_0, address_1) = if i_am_0 {
                                (crypto.own_address, from_address.clone())
                            } else {
                                (from_address.clone(), crypto.own_address)
                            };

                            Box::new(
                                blockchain_client
                                    .check_for_open(&address_0, &address_1)
                                    .and_then(move |channel_id| {
                                        *counterparty = Counterparty::Open {
                                            channel: CombinedState::new(&Channel {
                                                channel_id,
                                                address_0,
                                                address_1,
                                                total_balance: new_channel_tx.clone().balance_0
                                                    + new_channel_tx.clone().balance_1,
                                                balance_0: new_channel_tx.balance_0,
                                                balance_1: new_channel_tx.balance_1,
                                                sequence_number: 0u64.into(),
                                                settling_period_length: new_channel_tx
                                                    .settling_period_length,
                                                settling_period_end: 0u64.into(),
                                                settling_period_started: false,
                                                i_am_0,
                                            }),
                                            url,
                                        };
                                        Ok(())
                                    }),
                            ) as Box<Future<Item = (), Error = Error>>
                        }
                        _ => {
                            let error = GuacError::WrongState {
                                correct_state: "OtherCreating".to_string(),
                                current_state: format!("{:?}", counterparty.clone()),
                                action: "notify channel opened".to_string(),
                            };
                            return Box::new(future::err(error.into())) // TODO: Design a better set of errors, and when to use them
                                as Box<Future<Item = (), Error = Error>>;
                        }
                    }
                }),
        )
    }

    fn notify_re_draw(
        &self,
        from_address: Address,
        _to_url: String,
    ) -> Box<Future<Item = (), Error = Error>> {
        let storage = self.storage.clone();
        let blockchain_client = self.blockchain_client.clone();
        Box::new(
            storage
                .get_counterparty(from_address.clone())
                .and_then(move |mut counterparty| {
                    match counterparty.clone() {
                        Counterparty::OtherReDrawing {
                            url,
                            re_draw_tx,
                            channel,
                        } => Box::new(
                            blockchain_client
                                .check_for_re_draw(channel.my_state.channel_id)
                                .and_then(move |_| {
                                    *counterparty = Counterparty::Open {
                                        channel: CombinedState::new(&Channel {
                                            total_balance: re_draw_tx.new_balance_0.clone()
                                                + re_draw_tx.new_balance_1.clone(),
                                            balance_0: re_draw_tx.new_balance_0,
                                            balance_1: re_draw_tx.new_balance_1,
                                            sequence_number: re_draw_tx.sequence_number.clone(),
                                            ..channel.my_state
                                        }),
                                        url,
                                    };
                                    Ok(())
                                }),
                        ) as Box<Future<Item = (), Error = Error>>,
                        _ => {
                            let error = GuacError::WrongState {
                                correct_state: "OtherReDrawing".to_string(),
                                current_state: format!("{:?}", counterparty.clone()),
                                action: "notify redraw".to_string(),
                            };
                            return Box::new(future::err(error.into())) // TODO: Design a better set of errors, and when to use them
                                as Box<Future<Item = (), Error = Error>>;
                        }
                    }
                }),
        )
    }

    fn receive_payment(
        &self,
        from_address: Address,
        _to_url: String,
        update_tx: UpdateTx,
    ) -> Box<Future<Item = UpdateTx, Error = Error>> {
        let storage = self.storage.clone();
        Box::new(
            storage
                .get_counterparty(from_address.clone())
                .and_then(move |mut counterparty| {
                    match counterparty.clone() {
                        Counterparty::Open { url, mut channel } => {
                            Box::new(future::ok(()).and_then(move |_| {
                                let my_update_tx = channel.receive_payment(&update_tx);

                                *counterparty = Counterparty::Open { channel, url };

                                my_update_tx
                            }))
                                as Box<Future<Item = UpdateTx, Error = Error>>
                        }
                        _ => {
                            let error = GuacError::WrongState {
                                correct_state: "Open".to_string(),
                                current_state: format!("{:?}", counterparty.clone()),
                                action: "receive payment".to_string(),
                            };
                            return Box::new(future::err(error.into())) // TODO: Design a better set of errors, and when to use them
                                as Box<Future<Item = UpdateTx, Error = Error>>;
                        }
                    }
                }),
        )
    }
}
