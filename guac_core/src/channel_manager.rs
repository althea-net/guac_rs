use crate::channel::Channel;
// use crate::combined_state::CombinedState;
use crate::types::UpdateTx;
use crate::types::{Counterparty, GuacError, NewChannelTx, ReDrawTx};
use clarity::{Address, Signature};

use crate::crypto::Crypto;
use failure::Error;
use futures::{future, Future};
use num256::Uint256;
use qutex::Guard;

use crate::storage::Storage;
use std::sync::Arc;

/// Todo:
/// - Integrate sig verification
/// - Get to the bottom of balance discrepancies in tests
/// - Implement expiration timer in state machine
/// - Get rid of useless "register counterparty" step
/// - Deal with incorrect accrual in packet loss scenario

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

macro_rules! forbidden {
    ($expression:expr, $label:expr) => {
        if !($expression) {
            return future::err(
                GuacError::Forbidden {
                    message: $label.to_string(),
                }
                .into(),
            );
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

pub trait BlockchainApi {
    fn balance_of(&self) -> Box<Future<Item = Uint256, Error = Error>>;

    fn check_for_open(
        &self,
        address_0: &Address,
        address_1: &Address,
    ) -> Box<Future<Item = Option<[u8; 32]>, Error = Error>>;

    fn check_for_re_draw(&self, channel_id: [u8; 32]) -> Box<Future<Item = (), Error = Error>>;

    fn quick_deposit(&self, value: Uint256) -> Box<Future<Item = (), Error = Error>>;

    fn get_current_block(&self) -> Box<Future<Item = Uint256, Error = Error>>;

    fn deposit_then_new_channel(
        &self,
        amount: Uint256,
        new_channel_tx: NewChannelTx,
    ) -> Box<Future<Item = [u8; 32], Error = Error>>;

    fn deposit_then_re_draw(
        &self,
        amount: Uint256,
        re_draw_tx: ReDrawTx,
    ) -> Box<Future<Item = (), Error = Error>>;

    fn re_draw_then_withdraw(
        &self,
        amount: Uint256,
        re_draw_tx: ReDrawTx,
    ) -> Box<Future<Item = (), Error = Error>>;
}

pub trait UserApi {
    fn fill_channel(
        &self,
        their_address: Address,
        their_url: String,
        amount: Uint256,
    ) -> Box<Future<Item = (), Error = Error>>;

    fn make_payment(
        &self,
        their_address: Address,
        their_url: String,
        amount: Uint256,
    ) -> Box<Future<Item = (), Error = Error>>;

    fn withdraw(
        &self,
        their_address: Address,
        their_url: String,
        amount: Uint256,
    ) -> Box<Future<Item = (), Error = Error>>;

    fn check_accrual(&self, their_address: Address) -> Box<Future<Item = Uint256, Error = Error>>;

    fn check_my_balance(
        &self,
        their_address: Address,
    ) -> Box<Future<Item = Uint256, Error = Error>>;

    fn get_state(&self, their_address: Address) -> Box<Future<Item = Counterparty, Error = Error>>;
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
    ) -> Box<Future<Item = Option<Uint256>, Error = Error>>;
}

fn check_for_counterparty(
    counterparty: Option<Guard<Counterparty>>,
) -> Result<Guard<Counterparty>, Error> {
    let counterparty = counterparty.ok_or(GuacError::Error {
        message: "Cannot find counterparty".into(),
    })?;
    Ok(counterparty)
}

fn make_counterparty_if_none(
    their_address: Address,
    my_address: Address,
    storage: Arc<Box<Storage>>,
) -> impl FnOnce(Option<Guard<Counterparty>>) -> Box<Future<Item = Guard<Counterparty>, Error = Error>>
{
    move |counterparty| match counterparty {
        Some(counterparty) => Box::new(future::ok(counterparty)),
        None => Box::new(
            storage
                .new_counterparty(
                    their_address.clone(),
                    Counterparty::New {
                        i_am_0: my_address < their_address,
                    },
                )
                .and_then(move |_| {
                    storage
                        .get_counterparty(their_address.clone())
                        .and_then(|counterparty| {
                            Ok(counterparty.expect("counterparty should have been created"))
                        })
                }),
        ),
    }
}

impl UserApi for Guac {
    fn check_accrual(&self, their_address: Address) -> Box<Future<Item = Uint256, Error = Error>> {
        let storage = self.storage.clone();

        Box::new(
            storage
                .get_counterparty(their_address.clone())
                .and_then(check_for_counterparty)
                .and_then(|mut counterparty| match &mut *counterparty {
                    Counterparty::Open { channel, .. }
                    | Counterparty::ReDrawing { channel, .. }
                    | Counterparty::OtherReDrawing { channel, .. } => {
                        let accrual = channel.check_accrual();
                        Ok(accrual)
                    }
                    counterparty => {
                        let error = GuacError::WrongState {
                            correct_state: "Open".to_string(),
                            current_state: format!("{:?}", counterparty.clone()),
                            action: "check_accrual".to_string(),
                        };
                        return Err(error.into());
                    }
                }),
        )
    }

    fn check_my_balance(
        &self,
        their_address: Address,
    ) -> Box<Future<Item = Uint256, Error = Error>> {
        let storage = self.storage.clone();

        Box::new(
            storage
                .get_counterparty(their_address.clone())
                .and_then(check_for_counterparty)
                .and_then(|mut counterparty| match &mut *counterparty {
                    Counterparty::Open { channel, .. }
                    | Counterparty::ReDrawing { channel, .. }
                    | Counterparty::OtherReDrawing { channel, .. } => Ok(if channel.i_am_0 {
                        channel.balance_0.clone()
                    } else {
                        channel.balance_1.clone()
                    }),
                    counterparty => {
                        let error = GuacError::WrongState {
                            correct_state: "Open".to_string(),
                            current_state: format!("{:?}", counterparty.clone()),
                            action: "check_accrual".to_string(),
                        };
                        return Err(error.into());
                    }
                }),
        )
    }

    fn get_state(&self, their_address: Address) -> Box<Future<Item = Counterparty, Error = Error>> {
        let storage = self.storage.clone();

        Box::new(
            storage
                .get_counterparty(their_address.clone())
                .and_then(check_for_counterparty)
                .and_then(|counterparty| Ok(counterparty.clone())),
        )
    }

    fn fill_channel(
        &self,
        their_address: Address,
        their_url: String,
        amount: Uint256,
    ) -> Box<Future<Item = (), Error = Error>> {
        let counterparty_client = self.counterparty_client.clone();
        let blockchain_client = self.blockchain_client.clone();
        let storage = self.storage.clone();
        let crypto = self.crypto.clone();

        Box::new(
            storage
                .get_counterparty(their_address)
                .and_then(make_counterparty_if_none(
                    their_address,
                    crypto.own_address,
                    self.storage.clone(),
                ))
                .and_then(move |mut counterparty| {
                    match counterparty.clone() {
                        Counterparty::New { i_am_0 } => {
                            let my_address = crypto.own_address;

                            let (address_0, address_1) = if i_am_0 {
                                (my_address, their_address)
                            } else {
                                (their_address, my_address)
                            };

                            let (balance_0, balance_1) = if i_am_0 {
                                (amount.clone(), 0u64.into())
                            } else {
                                (0u64.into(), amount.clone())
                            };

                            Box::new(
                                blockchain_client
                                    .get_current_block()
                                    .and_then(move |block| {
                                        let new_channel_tx = NewChannelTx {
                                            address_0: address_0.clone(),
                                            address_1: address_1.clone(),
                                            balance_0: balance_0.clone(),
                                            balance_1: balance_1.clone(),
                                            expiration: (block + 40u64.into()), // current block plus 10 minutes
                                            settling_period_length: 5000u64.into(), //TODO: figure out default value
                                            signature_0: None,
                                            signature_1: None,
                                        };

                                        counterparty_client
                                            .propose_channel(
                                                my_address,
                                                their_url.clone(),
                                                new_channel_tx.clone(),
                                            )
                                            .and_then(move |their_signature| {
                                                let fingerprint = new_channel_tx
                                                    .clone()
                                                    .fingerprint(crypto.contract_address);

                                                let recovered_address = try_future_box!(
                                                    their_signature.recover(&fingerprint)
                                                );

                                                if recovered_address != their_address {
                                                    return Box::new(future::err(
                                                        GuacError::Error {
                                                            message: "Their signature is incorrect"
                                                                .into(),
                                                        }
                                                        .into(),
                                                    ));
                                                }

                                                let my_signature = crypto.eth_sign(
                                                    &new_channel_tx
                                                        .clone()
                                                        .fingerprint(crypto.contract_address),
                                                );

                                                let (signature_0, signature_1) = if i_am_0 {
                                                    (my_signature, their_signature)
                                                } else {
                                                    (their_signature, my_signature)
                                                };

                                                *counterparty = Counterparty::Creating {
                                                    new_channel_tx: new_channel_tx.clone(),
                                                    i_am_0,
                                                };

                                                Box::new(
                                                    blockchain_client
                                                        .deposit_then_new_channel(
                                                            amount.clone(),
                                                            NewChannelTx {
                                                                signature_0: Some(signature_0),
                                                                signature_1: Some(signature_1),
                                                                ..new_channel_tx
                                                            },
                                                        )
                                                        .and_then(move |channel_id| {
                                                            counterparty_client
                                                                .notify_channel_opened(
                                                                    my_address,
                                                                    their_url.clone(),
                                                                )
                                                                .and_then(move |()| {
                                                                    *counterparty =
                                                                        Counterparty::Open {
                                                                            channel: Channel {
                                                                                channel_id,
                                                                                sequence_number:
                                                                                    0u8.into(),
                                                                                balance_0,
                                                                                balance_1,
                                                                                i_am_0,
                                                                                accrual: 0u8.into(),
                                                                            },
                                                                        };
                                                                    Ok(())
                                                                })
                                                        }),
                                                )
                                            })
                                    }),
                            ) as Box<Future<Item = (), Error = Error>>
                        }
                        Counterparty::Open { channel } => {
                            let balance_0 = channel.balance_0.clone();
                            let balance_1 = channel.balance_1.clone();

                            let (new_balance_0, new_balance_1) = if channel.i_am_0 {
                                (balance_0 + amount.clone(), balance_1)
                            } else {
                                (balance_0, balance_1 + amount.clone())
                            };

                            Box::new(
                                blockchain_client
                                    .get_current_block()
                                    .and_then(move |block| {
                                        let re_draw_tx = ReDrawTx {
                                            channel_id: channel.channel_id.clone(),
                                            sequence_number: channel.sequence_number.clone()
                                                + 1u64.into(),
                                            old_balance_0: channel.balance_0.clone(),
                                            old_balance_1: channel.balance_1.clone(),
                                            new_balance_0: new_balance_0.clone(),
                                            new_balance_1: new_balance_1.clone(),
                                            expiration: (block + 40u64.into()), // current block plus 10 minutes
                                            signature_0: None,
                                            signature_1: None,
                                        };

                                        counterparty_client
                                            .propose_re_draw(
                                                crypto.own_address,
                                                their_url.clone(),
                                                re_draw_tx.clone(),
                                            )
                                            .and_then(move |their_signature| {
                                                let fingerprint = re_draw_tx
                                                    .clone()
                                                    .fingerprint(crypto.contract_address);

                                                let recovered_address = try_future_box!(
                                                    their_signature.recover(&fingerprint)
                                                );

                                                if recovered_address != their_address {
                                                    return Box::new(future::err(
                                                        GuacError::Error {
                                                            message: "Their signature is incorrect"
                                                                .into(),
                                                        }
                                                        .into(),
                                                    ));
                                                }

                                                *counterparty = Counterparty::ReDrawing {
                                                    channel: channel.clone(),
                                                    re_draw_tx: re_draw_tx.clone(),
                                                };

                                                let my_signature = crypto.eth_sign(
                                                    &re_draw_tx
                                                        .fingerprint(crypto.contract_address),
                                                );

                                                let (signature_0, signature_1) =
                                                    if channel.clone().i_am_0 {
                                                        (my_signature, their_signature)
                                                    } else {
                                                        (their_signature, my_signature)
                                                    };

                                                Box::new(
                                                    blockchain_client
                                                        .deposit_then_re_draw(
                                                            amount.clone(),
                                                            ReDrawTx {
                                                                signature_0: Some(signature_0),
                                                                signature_1: Some(signature_1),
                                                                ..re_draw_tx
                                                            },
                                                        )
                                                        .and_then(move |_| {
                                                            counterparty_client
                                                                .notify_re_draw(
                                                                    crypto.own_address,
                                                                    their_url.clone(),
                                                                )
                                                                .and_then(move |_| {
                                                                    // Save the new open state of the channel
                                                                    *counterparty =
                                                                        Counterparty::Open {
                                                                            channel: Channel {
                                                                                // TODO: what else changes here?
                                                                                balance_0:
                                                                                    new_balance_0
                                                                                        .clone(),
                                                                                balance_1:
                                                                                    new_balance_1
                                                                                        .clone(),
                                                                                ..channel
                                                                            },
                                                                        };
                                                                    Ok(())
                                                                })
                                                        }),
                                                )
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
                }),
        )
    }

    fn withdraw(
        &self,
        their_address: Address,
        their_url: String,
        amount: Uint256,
    ) -> Box<Future<Item = (), Error = Error>> {
        let storage = self.storage.clone();
        let counterparty_client = self.counterparty_client.clone();
        let blockchain_client = self.blockchain_client.clone();
        let crypto = self.crypto.clone();

        Box::new(
            storage
                .get_counterparty(their_address.clone())
                .and_then(check_for_counterparty)
                .and_then(move |mut counterparty| {
                    match counterparty.clone() {
                        Counterparty::Open { channel } => {
                            let balance_0 = channel.balance_0.clone();
                            let balance_1 = channel.balance_1.clone();

                            let (new_balance_0, new_balance_1) = if channel.i_am_0 {
                                (balance_0 - amount.clone(), balance_1)
                            } else {
                                (balance_0, balance_1 - amount.clone())
                            };

                            Box::new(
                                blockchain_client
                                    .get_current_block()
                                    .and_then(move |block| {
                                        let re_draw_tx = ReDrawTx {
                                            channel_id: channel.channel_id.clone(),
                                            sequence_number: channel.sequence_number.clone()
                                                + 1u64.into(),
                                            old_balance_0: channel.balance_0.clone(),
                                            old_balance_1: channel.balance_1.clone(),
                                            new_balance_0: new_balance_0.clone(),
                                            new_balance_1: new_balance_1.clone(),
                                            expiration: (block + 40u64.into()), // current block plus 10 minutes
                                            signature_0: None,
                                            signature_1: None,
                                        };

                                        counterparty_client
                                            .propose_re_draw(
                                                crypto.own_address,
                                                their_url.clone(),
                                                re_draw_tx.clone(),
                                            )
                                            .and_then(move |their_signature| {
                                                let fingerprint = re_draw_tx
                                                    .clone()
                                                    .fingerprint(crypto.contract_address);

                                                let recovered_address = try_future_box!(
                                                    their_signature.recover(&fingerprint)
                                                );

                                                if recovered_address != their_address {
                                                    return Box::new(future::err(
                                                        GuacError::Error {
                                                            message: "Their signature is incorrect"
                                                                .into(),
                                                        }
                                                        .into(),
                                                    ));
                                                }

                                                *counterparty = Counterparty::ReDrawing {
                                                    channel: channel.clone(),
                                                    re_draw_tx: re_draw_tx.clone(),
                                                };

                                                let my_signature = crypto.eth_sign(
                                                    &re_draw_tx
                                                        .fingerprint(crypto.contract_address),
                                                );

                                                let (signature_0, signature_1) =
                                                    if channel.clone().i_am_0 {
                                                        (my_signature, their_signature)
                                                    } else {
                                                        (their_signature, my_signature)
                                                    };

                                                Box::new(
                                                    blockchain_client
                                                        .re_draw_then_withdraw(
                                                            amount.clone(),
                                                            ReDrawTx {
                                                                signature_0: Some(signature_0),
                                                                signature_1: Some(signature_1),
                                                                ..re_draw_tx
                                                            },
                                                        )
                                                        .and_then(move |_| {
                                                            counterparty_client
                                                                .notify_re_draw(
                                                                    crypto.own_address,
                                                                    their_url.clone(),
                                                                )
                                                                .and_then(move |_| {
                                                                    // Save the new open state of the channel
                                                                    *counterparty =
                                                                        Counterparty::Open {
                                                                            channel: Channel {
                                                                                balance_0:
                                                                                    new_balance_0
                                                                                        .clone(),
                                                                                balance_1:
                                                                                    new_balance_1
                                                                                        .clone(),
                                                                                ..channel
                                                                            },
                                                                        };
                                                                    Ok(())
                                                                })
                                                        }),
                                                )
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
                }),
        )
    }

    fn make_payment(
        &self,
        their_address: Address,
        their_url: String,
        amount: Uint256,
    ) -> Box<Future<Item = (), Error = Error>> {
        let storage = self.storage.clone();
        let counterparty_client = self.counterparty_client.clone();
        let crypto = self.crypto.clone();

        Box::new(
            storage
                .get_counterparty(their_address.clone())
                .and_then(check_for_counterparty)
                .and_then(move |mut counterparty| match counterparty.clone() {
                    Counterparty::Open { mut channel } => {
                        let mut update_tx =
                            try_future_box!(channel.make_payment(amount.clone(), None));

                        let my_signature = crypto
                            .eth_sign(&update_tx.clone().fingerprint(crypto.contract_address));

                        if channel.i_am_0 {
                            update_tx.signature_0 = Some(my_signature);
                        } else {
                            update_tx.signature_1 = Some(my_signature);
                        };

                        Box::new(
                            counterparty_client
                                .receive_payment(
                                    crypto.own_address,
                                    their_url.clone(),
                                    update_tx.clone(),
                                )
                                .and_then(move |res: Option<Uint256>| {
                                    if let Some(current_seq) = res {
                                        let mut update_tx = try_future_box!(
                                            channel.make_payment(amount, Some(current_seq))
                                        );

                                        let my_signature = crypto.eth_sign(
                                            &update_tx.clone().fingerprint(crypto.contract_address),
                                        );

                                        if channel.i_am_0 {
                                            update_tx.signature_0 = Some(my_signature);
                                        } else {
                                            update_tx.signature_1 = Some(my_signature);
                                        };

                                        Box::new(
                                            counterparty_client
                                                .receive_payment(
                                                    crypto.own_address,
                                                    their_url.clone(),
                                                    update_tx.clone(),
                                                )
                                                .and_then(move |res: Option<Uint256>| {
                                                    if let Some(_) = res {
                                                        Err(GuacError::Error {
                                                            message: "Sequence number disagreement"
                                                                .to_string(),
                                                        }
                                                        .into())
                                                    } else {
                                                        Ok(())
                                                    }
                                                }),
                                        )
                                            as Box<Future<Item = (), Error = Error>>
                                    } else {
                                        *counterparty = Counterparty::Open { channel };
                                        Box::new(future::ok(()))
                                            as Box<Future<Item = (), Error = Error>>
                                    }
                                }),
                        ) as Box<Future<Item = (), Error = Error>>
                    }
                    _ => {
                        let error = GuacError::WrongState {
                            correct_state: "Open".to_string(),
                            current_state: format!("{:?}", counterparty.clone()),
                            action: "make payment".to_string(),
                        };
                        return Box::new(future::err(error.into()))
                            as Box<Future<Item = (), Error = Error>>;
                    }
                }),
        )
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
        let my_address = crypto.own_address;

        Box::new(
            storage
                .get_counterparty(from_address)
                .and_then(make_counterparty_if_none(
                    from_address,
                    crypto.own_address,
                    self.storage.clone(),
                ))
                .and_then(move |mut counterparty| {
                    match counterparty.clone() {
                        Counterparty::New { i_am_0 } => {
                            Box::new(
                                future::ok(())
                                    .and_then({
                                        let new_channel_tx = new_channel_tx.clone();
                                        move |_| {
                                            let NewChannelTx {
                                                address_0,
                                                address_1,
                                                balance_0,
                                                balance_1,
                                                expiration: _,
                                                settling_period_length,
                                                signature_0: _,
                                                signature_1: _,
                                            } = new_channel_tx.clone();

                                            if i_am_0 {
                                                forbidden!(
                                                    address_0 == my_address,
                                                    format!(
                                                    "Address 0 ({}) should equal my address ({})",
                                                    address_0.to_string(),
                                                    my_address.to_string()
                                                )
                                                );
                                                forbidden!(
                                                    address_1 == from_address,
                                                    format!(
                                                    "Address 1 ({}) should equal your address ({})",
                                                    address_1.to_string(),
                                                    from_address.to_string()
                                                )
                                                );
                                            } else {
                                                forbidden!(
                                                    address_1 == my_address,
                                                    format!(
                                                    "Address 1 ({}) should equal my address ({})",
                                                    address_1.to_string(),
                                                    my_address.to_string()
                                                )
                                                );
                                                forbidden!(
                                                    address_0 == from_address,
                                                    format!(
                                                    "Address 0 ({}) should equal your address ({})",
                                                    address_0.to_string(),
                                                    from_address.to_string()
                                                )
                                                );
                                            }

                                            let my_balance =
                                                if i_am_0 { balance_0 } else { balance_1 };

                                            forbidden!(
                                                my_balance == 0u64.into(),
                                                "My balance in proposed channel must be zero."
                                            );

                                            forbidden!(
                                                settling_period_length == 5000u64.into(),
                                                "I only accept settling periods of 5000 blocks"
                                            );

                                            future::ok(())
                                        }
                                    })
                                    .and_then({
                                        let crypto = crypto.clone();
                                        let new_channel_tx = new_channel_tx.clone();
                                        move |_| {
                                            // Save the current state of the counterparty
                                            *counterparty = Counterparty::OtherCreating {
                                                i_am_0,
                                                new_channel_tx: new_channel_tx.clone(),
                                            };

                                            let my_signature = crypto.eth_sign(
                                                &new_channel_tx
                                                    .fingerprint(crypto.contract_address),
                                            );
                                            Ok(my_signature)
                                        }
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
                            return Box::new(future::err(error.into()))
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
                .and_then(check_for_counterparty)
                .and_then(move |mut counterparty| match counterparty.clone() {
                    Counterparty::Open { channel } => {
                        let channel_clone_1 = channel.clone();
                        let re_draw_tx_clone_1 = re_draw_tx.clone();
                        Box::new(future::ok(()).and_then(move |_| {
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

                            forbidden!(
                                channel_id == channel.channel_id,
                                format!(
                                    "Channel ID ({:?}) should equal my saved channel ID ({:?})",
                                    channel_id, channel.channel_id
                                )
                            );

                            forbidden!(
                                sequence_number > channel.sequence_number,
                                format!(
                                    "Sequence number ({}) should be higher than {}",
                                    sequence_number, channel.sequence_number
                                )
                            );

                            forbidden!(
                                old_balance_0 == channel.balance_0,
                                format!(
                                    "Old balance_0 ({}) should equal {}",
                                    old_balance_0, channel.balance_0
                                )
                            );

                            forbidden!(
                                old_balance_1 == channel.balance_1,
                                format!(
                                    "Old balance_1 ({}) should equal {}",
                                    old_balance_1, channel.balance_1
                                )
                            );

                            if channel.i_am_0 {
                                forbidden!(
                                    new_balance_0 == channel.balance_0,
                                    format!(
                                        "New balance_0 ({}) should equal my balance ({})",
                                        new_balance_0, channel.balance_0
                                    )
                                );
                            } else {
                                forbidden!(
                                    new_balance_1 == channel.balance_1,
                                    format!(
                                        "New balance_1 ({}) should equal my balance ({})",
                                        new_balance_1, channel.balance_1
                                    )
                                );
                            }

                            *counterparty = Counterparty::OtherReDrawing {
                                channel: channel_clone_1,
                                re_draw_tx: re_draw_tx_clone_1.clone(),
                            };

                            let my_signature = crypto
                                .eth_sign(&re_draw_tx_clone_1.fingerprint(crypto.contract_address));

                            future::ok(my_signature)
                        })) as Box<Future<Item = Signature, Error = Error>>
                    }
                    _ => {
                        let error = GuacError::WrongState {
                            correct_state: "Open".to_string(),
                            current_state: format!("{:?}", counterparty.clone()),
                            action: "propose redraw".to_string(),
                        };
                        return Box::new(future::err(error.into()))
                            as Box<Future<Item = Signature, Error = Error>>;
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
                .and_then(check_for_counterparty)
                .and_then(move |mut counterparty| match counterparty.clone() {
                    Counterparty::OtherCreating {
                        i_am_0,
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
                                .and_then(move |maybe_channel_id| {
                                    if let Some(channel_id) = maybe_channel_id {
                                        *counterparty = Counterparty::Open {
                                            channel: Channel {
                                                channel_id,
                                                sequence_number: 0u64.into(),
                                                balance_0: new_channel_tx.balance_0,
                                                balance_1: new_channel_tx.balance_1,
                                                i_am_0,
                                                accrual: 0u64.into(),
                                            },
                                        };
                                        Ok(())
                                    } else {
                                        bail!("Cannot confirm that channel was opened");
                                    }
                                }),
                        ) as Box<Future<Item = (), Error = Error>>
                    }
                    _ => {
                        let error = GuacError::WrongState {
                            correct_state: "OtherCreating".to_string(),
                            current_state: format!("{:?}", counterparty.clone()),
                            action: "notify channel opened".to_string(),
                        };
                        return Box::new(future::err(error.into()))
                            as Box<Future<Item = (), Error = Error>>;
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
                .and_then(check_for_counterparty)
                .and_then(move |mut counterparty| match counterparty.clone() {
                    Counterparty::OtherReDrawing {
                        re_draw_tx,
                        channel,
                    } => Box::new(
                        blockchain_client
                            .check_for_re_draw(channel.channel_id)
                            .and_then(move |_| {
                                *counterparty = Counterparty::Open {
                                    channel: Channel {
                                        balance_0: re_draw_tx.new_balance_0,
                                        balance_1: re_draw_tx.new_balance_1,
                                        sequence_number: re_draw_tx.sequence_number.clone(),
                                        ..channel
                                    },
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
                        return Box::new(future::err(error.into()))
                            as Box<Future<Item = (), Error = Error>>;
                    }
                }),
        )
    }

    fn receive_payment(
        &self,
        from_address: Address,
        _to_url: String,
        update_tx: UpdateTx,
    ) -> Box<Future<Item = Option<Uint256>, Error = Error>> {
        let storage = self.storage.clone();
        let crypto = self.crypto.clone();
        Box::new(
            storage
                .get_counterparty(from_address.clone())
                .and_then(check_for_counterparty)
                .and_then(move |mut counterparty| match counterparty.clone() {
                    Counterparty::Open { mut channel } => {
                        Box::new(future::ok(()).and_then(move |_| {
                            let their_signature = if channel.i_am_0 {
                                update_tx.clone().signature_1
                            } else {
                                update_tx.clone().signature_0
                            };

                            let their_signature = match their_signature {
                                Some(sig) => sig,
                                None => {
                                    return Err(GuacError::Forbidden {
                                        message: "No signature supplied".into(),
                                    }
                                    .into())
                                }
                            };

                            let fingerprint =
                                update_tx.clone().fingerprint(crypto.contract_address);

                            let recovered_address = their_signature.recover(&fingerprint)?;

                            if recovered_address != from_address {
                                return Err(GuacError::Forbidden {
                                    message: "Your signature is incorrect".into(),
                                }
                                .into());
                            }

                            let maybe_seq = channel.receive_payment(&update_tx)?;

                            *counterparty = Counterparty::Open { channel };

                            Ok(maybe_seq)
                        }))
                            as Box<Future<Item = Option<Uint256>, Error = Error>>
                    }
                    _ => {
                        let error = GuacError::WrongState {
                            correct_state: "Open".to_string(),
                            current_state: format!("{:?}", counterparty.clone()),
                            action: "receive payment".to_string(),
                        };
                        return Box::new(future::err(error.into()))
                            as Box<Future<Item = Option<Uint256>, Error = Error>>;
                    }
                }),
        )
    }
}
