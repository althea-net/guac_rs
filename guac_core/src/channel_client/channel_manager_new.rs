use clarity::{Address, Signature};
use crypto::CryptoService;
use failure::Error;
use futures::{future, Future};
use num256::Uint256;
use qutex::Guard;
use std::sync::Arc;
use transport_protocol::TransportProtocol;
use {CRYPTO, STORAGE};
// use channel_client::Channel;

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

#[derive(Clone, Debug)]
pub struct Channel {
    pub channel_id: Uint256,
    pub address0: Address,
    pub address1: Address,

    pub total_balance: Uint256,
    pub balance0: Uint256,
    pub balance1: Uint256,
    pub sequence_number: Uint256,

    pub settling_period_length: Uint256,
    pub settling_period_started: bool,
    pub settling_period_end: Uint256,
}

#[derive(Clone, Debug)]
pub struct Counterparty {
    pub channel: Channel,
    pub state: ChannelState,
    pub url: String,
    pub i_am_0: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub enum ChannelState {
    New,
    Creating,
    OtherCreating,
    ReDrawing,
    OtherReDrawing,
    Open,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct NewChannelTx {
    pub address0: Address,
    pub address1: Address,

    pub balance0: Uint256,
    pub balance1: Uint256,

    pub expiration: Uint256,
    pub settling_period_length: Uint256,

    pub signature0: Option<Signature>,
    pub signature1: Option<Signature>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ReDrawTx {
    pub channel_id: Uint256,

    pub sequence_number: Uint256,
    pub old_balance0: Uint256,
    pub old_balance1: Uint256,

    pub new_balance0: Uint256,
    pub new_balance1: Uint256,

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
}

pub trait BlockchainClient {
    fn new_channel(&self, new_channel: &NewChannelTx)
        -> Box<Future<Item = Uint256, Error = Error>>;

    fn re_draw(&self, new_channel: &ReDrawTx) -> Box<Future<Item = Uint256, Error = Error>>;
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

impl Guac {
    fn fill_channel(
        mut self,
        their_address: Address,
        amount: Uint256,
    ) -> Box<Future<Item = (), Error = Error>> {
        let storage = self.storage.clone();
        let counterparty_client = self.counterparty_client.clone();
        let blockchain_client = self.blockchain_client.clone();
        Box::new(
            storage
                .get_counterparty(their_address.clone())
                .and_then(move |counterparty| {
                    match counterparty.state {
                        ChannelState::New => {
                            // Do propose_channel

                            let my_address = CRYPTO.own_eth_addr();

                            let i_am_0 = my_address < their_address;

                            let (address0, address1) = if (i_am_0) {
                                (my_address, their_address)
                            } else {
                                (their_address, my_address)
                            };

                            let (balance0, balance1) = if (i_am_0) {
                                (amount, 0.into())
                            } else {
                                (0.into(), amount)
                            };

                            let new_channel = NewChannelTx {
                                address0,
                                address1,
                                balance0,
                                balance1,
                                expiration: 9999999999.into(), //TODO: get current block plus some
                                settling_period_length: 5000.into(), //TODO: figure out default value
                                signature0: None,
                                signature1: None,
                            };

                            let my_signature = new_channel.sign();

                            Box::new(
                                counterparty_client
                                    .propose_channel(&new_channel)
                                    .and_then(move |their_signature| {
                                        let (signature0, signature1) = if (i_am_0) {
                                            (my_signature, their_signature)
                                        } else {
                                            (their_signature, my_signature)
                                        };

                                        counterparty.state = ChannelState::Creating;

                                        blockchain_client.new_channel(&NewChannelTx {
                                            signature0: Some(signature0),
                                            signature1: Some(signature1),
                                            ..new_channel
                                        })
                                    }).and_then(move |channel_id| {
                                        counterparty_client.notify_channel_opened(&channel_id)
                                    }).and_then(move |()| {
                                        counterparty.state = ChannelState::Open;
                                        Ok(())
                                    }),
                            ) as Box<Future<Item = (), Error = Error>>
                        }
                        _ => {
                            // Make user wait
                            return Box::new(future::err(GuacError::TryAgainLater().into()))
                                as Box<Future<Item = (), Error = Error>>;
                        }
                        ChannelState::Open => {
                            // Do redraw
                            let channel = counterparty.channel.clone();

                            let balance0 = channel.balance0.clone();
                            let balance1 = channel.balance1.clone();

                            let (new_balance0, new_balance1) = if (counterparty.i_am_0) {
                                (balance0 + amount, balance1)
                            } else {
                                (balance0, balance1 + amount)
                            };

                            let re_draw = ReDrawTx {
                                channel_id: channel.channel_id,
                                sequence_number: channel.sequence_number,
                                old_balance0: channel.balance0.clone(),
                                old_balance1: channel.balance1.clone(),
                                new_balance0,
                                new_balance1,
                                expiration: 9999999999.into(), //TODO: get current block plus some,
                                signature0: None,
                                signature1: None,
                            };

                            let my_signature = re_draw.sign();

                            Box::new(
                                counterparty_client
                                    .propose_re_draw(&re_draw)
                                    .and_then(move |their_signature| {
                                        counterparty.state = ChannelState::ReDrawing;
                                        storage.update_counterparty(counterparty);
                                        their_signature
                                    }).and_then(move |their_signature| {
                                        let (signature0, signature1) = if (counterparty.i_am_0) {
                                            (my_signature, their_signature)
                                        } else {
                                            (their_signature, my_signature)
                                        };
                                    }).and_then(move |(their_signature, my_signature)| {
                                        blockchain_client.re_draw(&ReDrawTx {
                                            signature0: Some(signature0),
                                            signature1: Some(signature1),
                                            ..re_draw
                                        })
                                    }).and_then(move |channel_id| {
                                        counterparty_client.notify_re_draw(&CRYPTO.own_eth_addr())
                                    }).and_then(move |()| {
                                        counterparty.state = ChannelState::Open;
                                        storage.update_counterparty(counterparty);
                                        Ok(())
                                    }),
                            ) as Box<Future<Item = (), Error = Error>>
                        }
                    }
                }),
        )
    }

    fn propose_channel(
        mut self,
        their_address: Address,
        new_channel_tx: &NewChannelTx,
    ) -> Box<Future<Item = Signature, Error = Error>> {
        let storage = self.storage.clone();
        let counterparty_client = self.counterparty_client.clone();
        let blockchain_client = self.blockchain_client.clone();
        Box::new(
            storage
                .get_counterparty(their_address.clone())
                .and_then(move |counterparty| match counterparty.state {
                    ChannelState::New => {
                        let NewChannelTx {
                            address0,
                            address1,
                            balance0,
                            balance1,
                            expiration,
                            settling_period_length,
                            signature0,
                            signature1,
                        } = new_channel_tx;

                        ensure!(address0 < address1, "Addresses must be sorted.");

                        let (my_balance, i_am_0) = if (address0 == CRYPTO.own_eth_addr()) {
                            (balance0, true)
                        } else if (address1 == CRYPTO.own_eth_addr()) {
                            (balance1, false)
                        } else {
                            bail!("This is NewChannelTx is not meant for me.")
                        };

                        ensure!(
                            my_balance == 0,
                            "My balance in proposed channel must be zero."
                        );

                        ensure!(
                            settling_period_length == 5000.into(),
                            "I only accept settling periods of 5000 blocks"
                        );

                        storage
                            .update_counterparty(Counterparty {
                                // Save the current state of the counterparty
                                channel: Channel {
                                    address0,
                                    address1,
                                    total_balance: balance0 + balance1,
                                    balance0,
                                    balance1,
                                    sequence_number: 0.into(),
                                    settling_period_length: 5000.into(),
                                    settling_period_started: false,
                                    settling_period_end: 0.into(),
                                },
                                state: ChannelState::OtherCreating,
                                i_am_0,
                                ..counterparty
                            }).and_then(|| {
                                // Return our signature
                                new_channel_tx.sign()
                            })
                    }
                    _ => {
                        // Can't do that in this state
                        return Box::new(future::err(GuacError::WrongState().into()))
                            as Box<Future<Item = (), Error = Error>>;
                    }
                }),
        )
    }

    fn propose_re_draw(
        mut self,
        their_address: Address,
        re_draw_tx: &ReDrawTx,
    ) -> Box<Future<Item = Signature, Error = Error>> {
        let storage = self.storage.clone();
        let counterparty_client = self.counterparty_client.clone();
        let blockchain_client = self.blockchain_client.clone();
        Box::new(
            storage
                .get_counterparty(their_address.clone())
                .and_then(move |counterparty| match counterparty.state {
                    ChannelState::Open => {
                        let channel = counterparty.channel;
                        let ReDrawTx {
                            channel_id,

                            sequence_number,
                            old_balance0,
                            old_balance1,

                            new_balance0,
                            new_balance1,

                            expiration,

                            signature0,
                            signature1,
                        } = re_draw_tx;

                        ensure!(channel_id == channel.channel_id, "Incorrect channel ID.");
                        ensure!(
                            sequence_number == channel.sequence_number,
                            "Incorrect sequence number."
                        );
                        ensure!(old_balance0 == channel.balance0, "Incorrect old balance0");
                        ensure!(old_balance1 == channel.balance1, "Incorrect old balance1");

                        if (i_am_0) {
                            ensure!(new_balance0 == channel.balance0, "Incorrect new balance0");
                        } else {
                            ensure!(new_balance1 == channel.balance1, "Incorrect new balance1");
                        }

                        storage
                            .update_counterparty(Counterparty {
                                // Save the current state of the counterparty
                                state: ChannelState::OtherReDrawing,
                                ..counterparty
                            }).and_then(|| {
                                // Return our signature
                                re_draw_tx.sign()
                            })
                    }
                    _ => {
                        // Can't do that in this state
                        return Box::new(future::err(GuacError::WrongState().into()))
                            as Box<Future<Item = (), Error = Error>>;
                    }
                }),
        )
    }
}
