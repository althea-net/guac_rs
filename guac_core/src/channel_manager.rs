use crate::channel::Channel;
use crate::crypto::Crypto;
use crate::storage::Storage;
use crate::types::{Counterparty, GuacError, NewChannelTx, ReDrawTx};
use crate::CounterpartyApi;
use clarity::Address;
use failure::Error;
use futures::{future, Future};
use num256::Uint256;
use qutex::Guard;
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

/// This will create an error if a counterparty cannot be found, or return the counterparty.Meant to
/// be used inside a futures chain.
pub fn check_for_counterparty(
    counterparty: Option<Guard<Counterparty>>,
) -> Result<Guard<Counterparty>, Error> {
    let counterparty = counterparty.ok_or(GuacError::Error {
        message: "Cannot find counterparty".into(),
    })?;
    Ok(counterparty)
}

/// This will create a counterparty if one cannot be found. Either way, it will return the
/// counterparty. Meant to be used in a futures chain.
pub fn make_counterparty_if_none(
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

impl Guac {
    pub fn check_accrual(
        &self,
        their_address: Address,
    ) -> impl Future<Item = Uint256, Error = Error> {
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

    pub fn check_my_balance(
        &self,
        their_address: Address,
    ) -> impl Future<Item = Uint256, Error = Error> {
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

    pub fn get_state(
        &self,
        their_address: Address,
    ) -> impl Future<Item = Counterparty, Error = Error> {
        let storage = self.storage.clone();

        Box::new(
            storage
                .get_counterparty(their_address.clone())
                .and_then(check_for_counterparty)
                .and_then(|counterparty| Ok(counterparty.clone())),
        )
    }

    pub fn fill_channel(
        &self,
        their_address: Address,
        their_url: String,
        amount: Uint256,
    ) -> impl Future<Item = (), Error = Error> {
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

    pub fn withdraw(
        &self,
        their_address: Address,
        their_url: String,
        amount: Uint256,
    ) -> impl Future<Item = (), Error = Error> {
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

    pub fn make_payment(
        &self,
        their_address: Address,
        their_url: String,
        amount: Uint256,
    ) -> impl Future<Item = (), Error = Error> {
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
