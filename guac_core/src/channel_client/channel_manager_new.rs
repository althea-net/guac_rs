use clarity::{Address, Signature};
use crypto::CryptoService;
use failure::Error;
use futures::Future;
use num256::Uint256;
use qutex::Guard;
use transport_protocol::TransportProtocol;
use {CRYPTO, STORAGE};
// use channel_client::Channel;

#[derive(Clone, Debug)]
pub struct ChannelManager {
    pub state: ChannelManagerState,
    pub channel: Option<Channel>,
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

    pub i_am_0: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub enum ChannelManagerState {
    New,
    // Creating,
    // OtherCreating,
    // ReDrawing,
    // OtherReDrawing,
    Open,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct NewChannel {
    pub address0: Address,
    pub address1: Address,

    pub balance0: Uint256,
    pub balance1: Uint256,

    pub expiration: Uint256,
    pub settlingPeriodLength: Uint256,

    pub signature0: Option<Signature>,
    pub signature1: Option<Signature>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ReDraw {
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

pub trait ChannelManagerFuncs {
    fn fill(
        mut self,
        their_address: Address,
        amount: Uint256,
    ) -> Box<Future<Item = (), Error = Error>>;
}

impl NewChannel {
    pub fn sign(&self) -> Signature {
        unimplemented!();
    }
}

impl ReDraw {
    pub fn sign(&self) -> Signature {
        unimplemented!();
    }
}

mod counterparty_client {
    use super::*;

    pub fn propose_channel(
        new_channel: &NewChannel,
    ) -> Box<Future<Item = Signature, Error = Error>> {
        unimplemented!();
    }

    pub fn propose_re_draw(re_draw: &ReDraw) -> Box<Future<Item = Signature, Error = Error>> {
        unimplemented!();
    }

    pub fn notify_channel_opened(channel_id: &Uint256) -> Box<Future<Item = (), Error = Error>> {
        unimplemented!();
    }

    pub fn notify_re_draw(my_address: &Address) -> Box<Future<Item = (), Error = Error>> {
        unimplemented!();
    }
}

mod blockchain_client {
    use super::*;

    pub fn new_channel(new_channel: &NewChannel) -> Box<Future<Item = Uint256, Error = Error>> {
        unimplemented!();
    }

    pub fn re_draw(new_channel: &ReDraw) -> Box<Future<Item = Uint256, Error = Error>> {
        unimplemented!();
    }
}

// mod apir {
//     use super::*;
//     pub fn fill(their_address: Address, amount: Uint256) -> Box<Future<Item = (), Error = Error>> {
//         Box::new(
//             STORAGE
//                 .get_channel(their_address)
//                 .and_then(move |mut channel_manager| {
//                     channel_manager.fill(&channel.data)?;
//                     Ok(())
//                 }),
//         )
//     }
// }

// Note: Due to the fact that we are using qutex::Guard,
impl ChannelManagerFuncs for Guard<ChannelManager> {
    fn fill(
        mut self,
        their_address: Address,
        amount: Uint256,
    ) -> Box<Future<Item = (), Error = Error>> {
        match self.state {
            ChannelManagerState::New => {
                // Do propose_channel

                let my_address = CRYPTO.own_eth_addr();

                let iAmAddress0 = my_address < their_address;

                let (address0, address1) = if (iAmAddress0) {
                    (my_address, their_address)
                } else {
                    (their_address, my_address)
                };

                let (balance0, balance1) = if (iAmAddress0) {
                    (amount, 0.into())
                } else {
                    (0.into(), amount)
                };

                let new_channel = NewChannel {
                    address0,
                    address1,
                    balance0,
                    balance1,
                    expiration: 9999999999.into(), //TODO: get current block plus some
                    settlingPeriodLength: 5000.into(), //TODO: figure out default value
                    signature0: None,
                    signature1: None,
                };

                let my_signature = new_channel.sign();

                Box::new(
                    counterparty_client::propose_channel(&new_channel)
                        .and_then(move |their_signature| {
                            let (signature0, signature1) = if (iAmAddress0) {
                                (my_signature, their_signature)
                            } else {
                                (their_signature, my_signature)
                            };

                            blockchain_client::new_channel(&NewChannel {
                                signature0: Some(signature0),
                                signature1: Some(signature1),
                                ..new_channel
                            })
                        }).and_then(|channel_id| {
                            counterparty_client::notify_channel_opened(&channel_id)
                        }).and_then(move |()| {
                            self.state = ChannelManagerState::Open;
                            Ok(())
                        }),
                )
            }
            // ChannelManagerState::Creating
            // | ChannelManagerState::OtherCreating
            // | ChannelManagerState::ReDrawing
            // | ChannelManagerState::OtherReDrawing => {
            //     // Do refill

            // }
            ChannelManagerState::Open => {
                let channel = self.clone().channel.unwrap().clone();

                let (new_balance0, new_balance1) = if (channel.i_am_0) {
                    (channel.balance0.clone() + amount, channel.balance1.clone())
                } else {
                    (channel.balance0.clone(), channel.balance1.clone() + amount)
                };

                // Do refill
                let re_draw = ReDraw {
                    channel_id: channel.channel_id.clone(),
                    sequence_number: channel.sequence_number.clone(),
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
                    counterparty_client::propose_re_draw(&re_draw)
                        .and_then(move |their_signature| {
                            let (signature0, signature1) = if (channel.i_am_0) {
                                (my_signature, their_signature)
                            } else {
                                (their_signature, my_signature)
                            };

                            blockchain_client::re_draw(&ReDraw {
                                signature0: Some(signature0),
                                signature1: Some(signature1),
                                ..re_draw
                            })
                        }).and_then(|channel_id| {
                            counterparty_client::notify_re_draw(&CRYPTO.own_eth_addr())
                        }).and_then(move |()| {
                            self.state = ChannelManagerState::Open;
                            Ok(())
                        }),
                )
            }
        }
    }
}
