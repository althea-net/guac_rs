use channel_client::combined_state::CombinedState;
use channel_client::types::UpdateTx;
use channel_client::Channel;
use clarity::{Address, Signature};
use crypto::CryptoService;
use failure::Error;
use futures::{future, Future};
use num256::Uint256;
use qutex::Guard;
use std::sync::Arc;
use transport_protocol::TransportProtocol;
use {CRYPTO, STORAGE};

pub struct Guac {
    blockchain_client: Arc<Box<BlockchainClient>>,
    counterparty_client: Arc<Box<CounterpartyClient>>,
    storage: Arc<Box<Storage>>,
}

#[derive(Debug, Fail)]
pub enum GuacError {
    #[fail(
        display = "Guac is currently waiting on another operation to complete. Try again later."
    )]
    TryAgainLater(),
    #[fail(display = "Cannot call this method in the current state.")]
    WrongState(),
}

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

pub trait CounterpartyClient {
    fn propose_channel(
        &self,
        new_channel: &NewChannelTx,
    ) -> Box<Future<Item = Signature, Error = Error>>;

    fn propose_re_draw(&self, re_draw: &ReDrawTx) -> Box<Future<Item = Signature, Error = Error>>;

    fn notify_channel_opened(&self, channel_id: &Uint256) -> Box<Future<Item = (), Error = Error>>;

    fn notify_re_draw(&self, my_address: &Address) -> Box<Future<Item = (), Error = Error>>;

    fn rec_payment(&self, update_tx: &UpdateTx) -> Box<Future<Item = UpdateTx, Error = Error>>;
}

pub trait BlockchainClient {
    fn new_channel(&self, new_channel: &NewChannelTx)
        -> Box<Future<Item = Uint256, Error = Error>>;

    fn re_draw(&self, new_channel: &ReDrawTx) -> Box<Future<Item = Uint256, Error = Error>>;

    fn check_for_open(
        &self,
        address_0: &Address,
        address_1: &Address,
    ) -> Box<Future<Item = Uint256, Error = Error>>;

    fn check_for_re_draw(&self, channel_id: &Uint256)
        -> Box<Future<Item = Uint256, Error = Error>>;
}

pub trait Storage {
    /// Creates a new channel for given parameters of a counterparty.
    // fn register_channel(&self, channel: Channel) -> Box<Future<Item = Channel, Error = Error>>;
    /// Get channel struct for a given channel ID.
    ///
    /// - `channel_id` - A valid channel ID
    fn get_counterparty(&self, address: Address)
        -> Box<Future<Item = Counterparty, Error = Error>>;
    /// Update a channel by its ID
    fn update_counterparty(
        &self,
        counterparty: Counterparty,
    ) -> Box<Future<Item = (), Error = Error>>;
}

pub trait UserApi {
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
        their_address: Address,
        new_channel_tx: NewChannelTx,
    ) -> Box<Future<Item = Signature, Error = Error>>;

    fn propose_re_draw(
        &self,
        their_address: Address,
        re_draw_tx: ReDrawTx,
    ) -> Box<Future<Item = Signature, Error = Error>>;

    fn notify_channel_opened(
        &self,
        their_address: Address,
    ) -> Box<Future<Item = (), Error = Error>>;

    fn notify_re_draw(&self, their_address: Address) -> Box<Future<Item = (), Error = Error>>;
}

impl UserApi for Guac {
    fn fill_channel(
        &self,
        their_address: Address,
        amount: Uint256,
    ) -> Box<Future<Item = (), Error = Error>> {
        let storage = self.storage.clone();

        Box::new(
            storage
                .get_counterparty(their_address.clone())
                .and_then(move |counterparty| {
                    match counterparty {
                        Counterparty::New { i_am_0, url } => {
                            self.do_propose_channel(their_address, amount, i_am_0, url)
                        }
                        Counterparty::Open { channel, url } => {
                            self.do_re_draw(their_address, amount, channel, url)
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
        amount: Uint256,
    ) -> Box<Future<Item = (), Error = Error>> {
        let storage = self.storage.clone();

        Box::new(
            storage
                .get_counterparty(their_address.clone())
                .and_then(move |counterparty| {
                    match counterparty {
                        Counterparty::Open { channel, url } => {
                            self.do_payment(their_address, amount, channel, url)
                        }
                        _ => {
                            // Make user wait
                            return Box::new(future::err(GuacError::TryAgainLater().into())) // TODO: Design a better set of errors, and when to use them
                                as Box<Future<Item = (), Error = Error>>;
                        }
                    }
                }),
        )
    }
}

impl Guac {
    fn do_propose_channel(
        &self,
        their_address: Address,
        amount: Uint256,
        i_am_0: bool,
        url: String,
    ) -> Box<Future<Item = (), Error = Error>> {
        let counterparty_client = self.counterparty_client.clone();
        let blockchain_client = self.blockchain_client.clone();
        let storage = self.storage.clone();

        let my_address = CRYPTO.own_eth_addr();

        let (address_0, address_1) = if i_am_0 {
            (my_address, their_address)
        } else {
            (their_address, my_address)
        };

        let (balance_0, balance_1) = if i_am_0 {
            (amount, 0.into())
        } else {
            (0.into(), amount)
        };

        let new_channel_tx = NewChannelTx {
            address_0: address_0.clone(),
            address_1: address_1.clone(),
            balance_0: balance_0.clone(),
            balance_1: balance_1.clone(),
            expiration: 9999999999.into(), //TODO: get current block plus some
            settling_period_length: 5000.into(), //TODO: figure out default value
            signature0: None,
            signature1: None,
        };

        let my_signature = new_channel_tx.clone().sign();

        Box::new(
            counterparty_client
                .propose_channel(&new_channel_tx)
                .and_then(move |their_signature| {
                    let (signature0, signature1) = if i_am_0 {
                        (my_signature, their_signature)
                    } else {
                        (their_signature, my_signature)
                    };

                    storage
                        .update_counterparty(Counterparty::Creating {
                            new_channel_tx: new_channel_tx.clone(),
                            i_am_0,
                            url: url.clone(),
                        }).and_then(move |_| {
                            blockchain_client.new_channel(&NewChannelTx {
                                signature0: Some(signature0),
                                signature1: Some(signature1),
                                ..new_channel_tx
                            })
                        }).and_then(move |channel_id| {
                            counterparty_client
                                .notify_channel_opened(&channel_id)
                                .and_then(move |()| {
                                    storage
                                        .update_counterparty(Counterparty::Open {
                                            channel: CombinedState::new(&Channel {
                                                channel_id,
                                                address_0,
                                                address_1,
                                                total_balance: balance_0.clone()
                                                    + balance_1.clone(),
                                                balance_0,
                                                balance_1,
                                                sequence_number: 0.into(),
                                                settling_period_length: 5000.into(),
                                                settling_period_started: false,
                                                settling_period_end: 0.into(),
                                                i_am_0,
                                            }),
                                            url: url,
                                        }).and_then(move |_| Ok(()))
                                })
                        })
                }),
        ) as Box<Future<Item = (), Error = Error>>
    }

    fn do_re_draw(
        &self,
        their_address: Address,
        amount: Uint256,
        channel: CombinedState,
        url: String,
    ) -> Box<Future<Item = (), Error = Error>> {
        let counterparty_client = self.counterparty_client.clone();
        let blockchain_client = self.blockchain_client.clone();
        let storage = self.storage.clone();

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
            expiration: 9999999999.into(), //TODO: get current block plus some,
            signature0: None,
            signature1: None,
        };

        let my_signature = re_draw_tx.sign();

        Box::new(counterparty_client.propose_re_draw(&re_draw_tx).and_then(
            move |their_signature| {
                storage
                    .update_counterparty(Counterparty::ReDrawing {
                        channel: channel.clone(),
                        re_draw_tx: re_draw_tx.clone(),
                        // i_am_0,
                        url: url.clone(),
                    }).and_then(move |_| {
                        let (signature0, signature1) = if channel.my_state.i_am_0 {
                            (my_signature, their_signature)
                        } else {
                            (their_signature, my_signature)
                        };

                        blockchain_client.re_draw(&ReDrawTx {
                            signature0: Some(signature0),
                            signature1: Some(signature1),
                            ..re_draw_tx
                        })
                    }).and_then(move |_| counterparty_client.notify_re_draw(&CRYPTO.own_eth_addr()))
                    .and_then(move |_| {
                        // Save the new open state of the channel
                        storage.clone().update_counterparty(Counterparty::Open {
                            // i_am_0,
                            url: url.clone(),
                            channel: CombinedState::new(&Channel {
                                // TODO: what else changes here?
                                balance_0: new_balance_0.clone(),
                                balance_1: new_balance_1.clone(),
                                ..channel.my_state
                            }),
                        })
                    })
            },
        ))
    }

    fn do_payment(
        &self,
        their_address: Address,
        amount: Uint256,
        channel: CombinedState,
        url: String,
    ) -> Box<Future<Item = (), Error = Error>> {
        let counterparty_client = self.counterparty_client.clone();
        Box::new(
            future::ok(())
                .and_then(|_| {
                    // TODO: add not enough money error
                    channel.pay_counterparty(amount);
                    let update_tx = channel.create_update()?;

                    Ok(update_tx)
                }).and_then(|update_tx| {
                    counterparty_client
                        .rec_payment(&update_tx)
                        .and_then(|their_update_tx| {
                            channel.received_updated_state(&their_update_tx)
                        })
                }),
        )
    }
}

impl CounterpartyApi for Guac {
    fn propose_channel(
        &self,
        their_address: Address,
        new_channel_tx: NewChannelTx,
    ) -> Box<Future<Item = Signature, Error = Error>> {
        let storage = self.storage.clone();
        let counterparty_client = self.counterparty_client.clone();
        let blockchain_client = self.blockchain_client.clone();
        let new_channel_tx_clone_1 = new_channel_tx.clone();
        let new_channel_tx_clone_2 = new_channel_tx.clone();

        Box::new(
            storage
                .get_counterparty(their_address.clone())
                .and_then(move |counterparty| match counterparty {
                    Counterparty::New { url, i_am_0 } => {
                        Box::new(
                            future::ok(())
                                .and_then(move |_| {
                                    let NewChannelTx {
                                        address_0,
                                        address_1,
                                        balance_0,
                                        balance_1,
                                        expiration,
                                        settling_period_length,
                                        signature0,
                                        signature1,
                                    } = new_channel_tx_clone_1;

                                    ensure!(address_0 < address_1, "Addresses must be sorted.");

                                    let (my_balance, i_am_0) = if address_0 == CRYPTO.own_eth_addr()
                                    {
                                        (balance_0, true)
                                    } else if address_1 == CRYPTO.own_eth_addr() {
                                        (balance_1, false)
                                    } else {
                                        bail!("This is NewChannelTx is not meant for me.")
                                    };

                                    ensure!(
                                        my_balance == 0.into(),
                                        "My balance in proposed channel must be zero."
                                    );

                                    ensure!(
                                        settling_period_length == 5000.into(),
                                        "I only accept settling periods of 5000 blocks"
                                    );
                                    Ok(())
                                }).and_then(move |_| {
                                    // Save the current state of the counterparty
                                    storage
                                        .update_counterparty(Counterparty::OtherCreating {
                                            i_am_0,
                                            new_channel_tx: new_channel_tx_clone_2.clone(),
                                            url,
                                        }).and_then(move |_| {
                                            // Return our signature
                                            Ok(new_channel_tx_clone_2.sign())
                                        })
                                }),
                        ) as Box<Future<Item = Signature, Error = Error>>
                    }
                    _ => {
                        // Can't do that in this state
                        Box::new(future::err(GuacError::WrongState().into()))
                            as Box<Future<Item = Signature, Error = Error>>
                    }
                }),
        )
    }

    fn propose_re_draw(
        &self,
        their_address: Address,
        re_draw_tx: ReDrawTx,
    ) -> Box<Future<Item = Signature, Error = Error>> {
        let storage = self.storage.clone();
        let counterparty_client = self.counterparty_client.clone();
        let blockchain_client = self.blockchain_client.clone();
        Box::new(
            storage
                .get_counterparty(their_address.clone())
                .and_then(move |counterparty| match counterparty {
                    Counterparty::Open {
                        url,
                        // i_am_0,
                        channel,
                    } => {
                        let channel_clone_1 = channel.clone();
                        let re_draw_tx_clone_1 = re_draw_tx.clone();
                        Box::new(
                            // Have to do this weird thing with future::ok to get ensure! to work
                            future::ok(())
                                .and_then(move |_| {
                                    let ReDrawTx {
                                        channel_id,

                                        sequence_number,
                                        old_balance_0,
                                        old_balance_1,

                                        new_balance_0,
                                        new_balance_1,

                                        expiration,

                                        signature0,
                                        signature1,
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
                                    Ok(())
                                }).and_then(move |_| {
                                    storage
                                        .update_counterparty(Counterparty::OtherReDrawing {
                                            channel: channel_clone_1,
                                            re_draw_tx: re_draw_tx_clone_1.clone(),
                                            url,
                                        }).and_then(move |_| {
                                            // Return our signature
                                            Ok(re_draw_tx_clone_1.sign())
                                        })
                                }),
                        ) as Box<Future<Item = Signature, Error = Error>>
                    }
                    _ => {
                        // Can't do that in this state
                        Box::new(future::err(GuacError::WrongState().into()))
                            as Box<Future<Item = Signature, Error = Error>>
                    }
                }),
        )
    }

    fn notify_channel_opened(
        &self,
        their_address: Address,
    ) -> Box<Future<Item = (), Error = Error>> {
        let storage = self.storage.clone();
        let blockchain_client = self.blockchain_client.clone();
        Box::new(
            storage
                .get_counterparty(their_address.clone())
                .and_then(move |counterparty| match counterparty {
                    Counterparty::OtherCreating {
                        i_am_0,
                        url,
                        new_channel_tx,
                    } => {
                        let (address_0, address_1) = if i_am_0 {
                            (CRYPTO.own_eth_addr(), their_address.clone())
                        } else {
                            (their_address.clone(), CRYPTO.own_eth_addr())
                        };

                        Box::new(
                            blockchain_client
                                .check_for_open(&address_0, &address_1)
                                .and_then(move |channel_id| {
                                    storage.update_counterparty(Counterparty::Open {
                                        channel: CombinedState::new(&Channel {
                                            channel_id,
                                            address_0,
                                            address_1,
                                            total_balance: new_channel_tx.clone().balance_0
                                                + new_channel_tx.clone().balance_1,
                                            balance_0: new_channel_tx.balance_0,
                                            balance_1: new_channel_tx.balance_1,
                                            sequence_number: 0.into(),
                                            settling_period_length: new_channel_tx
                                                .settling_period_length,
                                            settling_period_end: 0.into(),
                                            settling_period_started: false,
                                            i_am_0,
                                        }),
                                        url,
                                    })
                                }),
                        ) as Box<Future<Item = (), Error = Error>>
                    }
                    _ => {
                        // Can't do that in this state
                        Box::new(future::err(GuacError::WrongState().into()))
                            as Box<Future<Item = (), Error = Error>>
                    }
                }),
        )
    }

    fn notify_re_draw(&self, their_address: Address) -> Box<Future<Item = (), Error = Error>> {
        let storage = self.storage.clone();
        let blockchain_client = self.blockchain_client.clone();
        Box::new(
            storage
                .get_counterparty(their_address.clone())
                .and_then(move |counterparty| match counterparty {
                    Counterparty::OtherReDrawing {
                        url,
                        re_draw_tx,
                        channel,
                    } => Box::new(
                        blockchain_client
                            .check_for_re_draw(&channel.my_state.channel_id)
                            .and_then(move |_| {
                                storage.update_counterparty(Counterparty::Open {
                                    channel: CombinedState::new(&Channel {
                                        total_balance: re_draw_tx.new_balance_0.clone()
                                            + re_draw_tx.new_balance_1.clone(),
                                        balance_0: re_draw_tx.new_balance_0,
                                        balance_1: re_draw_tx.new_balance_1,
                                        sequence_number: re_draw_tx.sequence_number.clone(),
                                        ..channel.my_state
                                    }),
                                    url,
                                })
                            }),
                    ) as Box<Future<Item = (), Error = Error>>,
                    _ => {
                        // Can't do that in this state
                        Box::new(future::err(GuacError::WrongState().into()))
                            as Box<Future<Item = (), Error = Error>>
                    }
                }),
        )
    }
}
