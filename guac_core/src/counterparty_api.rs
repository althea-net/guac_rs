use channel::Channel;
use channel_manager::check_for_counterparty;
use channel_manager::make_counterparty_if_none;
use clarity::{Address, Signature};
use failure::Error;
use futures::{future, Future};
use num256::Uint256;
use types::UpdateTx;
use types::{Counterparty, GuacError, NewChannelTx, ReDrawTx};
use Guac;

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
