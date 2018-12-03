use channel_client::types::UpdateTx;
use clarity::abi::encode_tokens;
use clarity::abi::{encode_call, Token};
use clarity::utils::hex_str_to_bytes;
use clarity::Transaction;
use clarity::{Address, Signature};
use error::GuacError;
use failure::Error;
use futures::future::ok;
use futures::Future;
use futures::IntoFuture;
use num256::Uint256;
use std::io::Cursor;

use crypto::{Action, CryptoService};
use payment_contract::{ChannelId, PaymentContract};
use CRYPTO;

pub struct Fullnode {
    pub address: Address,
    pub url: String,
}

pub fn create_update_fingerprint_data(
    contract_address: &Address,
    channel_id: &ChannelId,
    nonce: &Uint256,
    balance0: &Uint256,
    balance1: &Uint256,
) -> Vec<u8> {
    let mut msg = "updateState".as_bytes().to_vec();
    msg.extend(contract_address.as_bytes());
    msg.extend(channel_id.to_vec());
    msg.extend(&{
        let data: [u8; 32] = nonce.clone().into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = balance0.clone().into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = balance1.clone().into();
        data
    });
    msg
}

pub fn create_new_channel_fingerprint_data(
    contract_address: &Address,
    address0: &Address,
    address1: &Address,
    balance0: &Uint256,
    balance1: &Uint256,
    expiration: &Uint256,
    settling: &Uint256,
) -> Vec<u8> {
    let mut msg = "newChannel".as_bytes().to_vec();
    msg.extend(contract_address.clone().as_bytes());
    msg.extend(address0.as_bytes());
    msg.extend(address1.as_bytes());
    msg.extend(&{
        let data: [u8; 32] = balance0.clone().into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = balance1.clone().into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = expiration.clone().into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = settling.clone().into();
        data
    });
    msg
}

pub fn create_close_channel_fast_fingerprint_data(
    contract_address: &Address,
    channel_id: &ChannelId,
    sequence_number: &Uint256,
    balance0: &Uint256,
    balance1: &Uint256,
) -> Vec<u8> {
    let mut msg = "closeChannelFast".as_bytes().to_vec();
    msg.extend(contract_address.as_bytes());
    msg.extend(channel_id.to_vec());
    msg.extend(&{
        let data: [u8; 32] = sequence_number.clone().into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = balance0.clone().into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = balance1.clone().into();
        data
    });
    msg
}

pub fn create_redraw_fingerprint_data(
    contract_addres: &Address,
    channel_id: &ChannelId,
    sequence_number: &Uint256,
    old_balance_a: &Uint256,
    old_balance_b: &Uint256,
    new_balance_a: &Uint256,
    new_balance_b: &Uint256,
    expiration: &Uint256,
) -> Vec<u8> {
    let mut msg = "reDraw".as_bytes().to_vec();
    msg.extend(contract_addres.as_bytes());
    msg.extend_from_slice(&channel_id[..]);
    msg.extend(&{
        let data: [u8; 32] = sequence_number.clone().into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = old_balance_a.clone().into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = old_balance_b.clone().into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = new_balance_a.clone().into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = new_balance_b.clone().into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = expiration.clone().into();
        data
    });
    msg
}

pub fn create_update_with_bounty_fingerprint_data(
    contract_address: &Address,
    channel_id: &ChannelId,
    sequence_number: &Uint256,
    balance0: &Uint256,
    balance1: &Uint256,
    signature0: &Signature,
    signature1: &Signature,
    bounty_amount: &Uint256,
) -> Vec<u8> {
    let mut msg = "updateStateWithBounty".as_bytes().to_vec();
    msg.extend(contract_address.clone().as_bytes());
    msg.extend_from_slice(&channel_id[..]);
    msg.extend(&{
        let data: [u8; 32] = sequence_number.clone().into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = balance0.clone().into();
        data
    });
    msg.extend(&{
        let data: [u8; 32] = balance1.clone().into();
        data
    });
    msg.extend(signature0.clone().into_bytes().to_vec());
    msg.extend(signature1.clone().into_bytes().to_vec());
    msg.extend(&{
        let data: [u8; 32] = bounty_amount.clone().into();
        data
    });
    msg
}
pub struct GuacContract;

impl GuacContract {
    pub fn new() -> Self {
        Self {}
    }
}

impl PaymentContract for GuacContract {
    fn quick_deposit(&self, value: Uint256) -> Box<Future<Item = (), Error = Error>> {
        let payload = encode_call("quickDeposit()", &[]);
        let call = CRYPTO
            .broadcast_transaction(Action::Call(payload), value)
            .map(|_| ());
        Box::new(call)
    }
    fn withdraw(&self, value: Uint256) -> Box<Future<Item = (), Error = Error>> {
        let payload = encode_call("withdraw(uint256)", &[value.into()]);
        let call = CRYPTO
            .broadcast_transaction(Action::Call(payload), 0u64.into())
            .map(|_| ());
        Box::new(call)
    }
    /// Calls ChannelOpen on the contract and waits for event.
    ///
    /// * `channel_id` - Channel ID
    /// * `address0` - Source address
    /// * `address1` - Destination address
    /// * `balance0` - Source balance (own balance)
    /// * `balance1` - Other party initial balance
    /// * `signature0` - Fingerprint signed by source address
    /// * `signature1` - Fingerprint signed by destination address
    /// * `expiration` - Block number which this call will be expired
    /// * `settling_period` - Max. blocks for a settling period to finish
    fn new_channel(
        &self,
        address0: Address,
        address1: Address,
        balance0: Uint256,
        balance1: Uint256,
        signature0: Signature,
        signature1: Signature,
        expiration: Uint256,
        settling_period: Uint256,
    ) -> Box<Future<Item = ChannelId, Error = Error>> {
        // Broadcast a transaction on the network with data
        assert!(address0 != address1, "Unable to open channel to yourself");

        // Reorder addresses
        let (address0, address1, balance0, balance1, signature0, signature1) =
            if address0 > address1 {
                (
                    address1, address0, balance1, balance0, signature1, signature0,
                )
            } else {
                (
                    address0, address1, balance0, balance1, signature0, signature1,
                )
            };
        assert!(address0 < address1);

        let addr0_bytes: [u8; 32] = {
            let mut data: [u8; 32] = Default::default();
            data[12..].copy_from_slice(&address0.as_bytes());
            data
        };
        let addr1_bytes: [u8; 32] = {
            let mut data: [u8; 32] = Default::default();
            data[12..].copy_from_slice(&address1.as_bytes());
            data
        };
        let event = CRYPTO.wait_for_event(
            "ChannelOpened(address,address,bytes32)",
            Some(vec![addr0_bytes]),
            Some(vec![addr1_bytes]),
        );

        let payload = encode_call(
            "newChannel(address,address,uint256,uint256,uint256,uint256,bytes,bytes)",
            &[
                // address0
                address0.into(),
                // address1
                address1.into(),
                // balance0
                balance0.into(),
                // balance1
                balance1.into(),
                // expiration
                expiration.into(),
                // settlingPeriodLength in blocks
                settling_period.into(),
                // signature0
                signature0.into_bytes().to_vec().into(),
                // signature1
                signature1.into_bytes().to_vec().into(),
            ],
        );
        let call = CRYPTO.broadcast_transaction(Action::Call(payload), 0u64.into());

        Box::new(
            call.join(event)
                .and_then(|(_tx, response)| {
                    // let response = response.get(0).unwrap();
                    let mut data: [u8; 32] = Default::default();
                    ensure!(
                        response.data.len() == 32,
                        "Invalid data length in ChannelOpened event"
                    );
                    data.copy_from_slice(&response.data);
                    Ok(data)
                }).into_future(),
        )
    }

    fn update_state(
        &self,
        channel_id: ChannelId,
        channel_nonce: Uint256,
        balance_a: Uint256,
        balance_b: Uint256,
        sig_a: Signature,
        sig_b: Signature,
    ) -> Box<Future<Item = (), Error = Error>> {
        let data = encode_call(
            "updateState(bytes32,uint256,uint256,uint256,bytes,bytes)",
            &[
                Token::Bytes(channel_id.to_vec()),
                channel_nonce.into(),
                balance_a.into(),
                balance_b.into(),
                sig_a.into_bytes().to_vec().into(),
                sig_b.into_bytes().to_vec().into(),
            ],
        );
        Box::new(
            CRYPTO
                .broadcast_transaction(Action::Call(data), Uint256::from(0u64))
                .and_then(|_tx| Ok(()))
                .into_future(),
        )
    }
    fn update_state_with_bounty(
        &self,
        channel_id: ChannelId,
        channel_nonce: Uint256,
        balance_a: Uint256,
        balance_b: Uint256,
        sig_a: Signature,
        sig_b: Signature,
        bounty_amount: Uint256,
        bounty_signature: Signature,
    ) -> Box<Future<Item = (), Error = Error>> {
        let data = encode_call(
            "updateStateWithBounty(bytes32,uint256,uint256,uint256,bytes,bytes,uint256,bytes)",
            &[
                Token::Bytes(channel_id.to_vec()),
                channel_nonce.into(),
                balance_a.into(),
                balance_b.into(),
                sig_a.into_bytes().to_vec().into(),
                sig_b.into_bytes().to_vec().into(),
                bounty_amount.into(),
                bounty_signature.into_bytes().to_vec().into(),
            ],
        );
        Box::new(
            CRYPTO
                .broadcast_transaction(Action::Call(data), Uint256::from(0u64))
                .and_then(|_tx| Ok(()))
                .into_future(),
        )
    }

    fn close_channel_fast(
        &self,
        channel_id: ChannelId,
        channel_nonce: Uint256,
        balance_a: Uint256,
        balance_b: Uint256,
        sig_a: Signature,
        sig_b: Signature,
    ) -> Box<Future<Item = (), Error = Error>> {
        let data = encode_call(
            "closeChannelFast(bytes32,uint256,uint256,uint256,bytes,bytes)",
            &[
                Token::Bytes(channel_id.to_vec()),
                channel_nonce.into(),
                balance_a.into(),
                balance_b.into(),
                sig_a.into_bytes().to_vec().into(),
                sig_b.into_bytes().to_vec().into(),
            ],
        );
        Box::new(
            CRYPTO
                .broadcast_transaction(Action::Call(data), Uint256::from(0u64))
                .and_then(|_tx| Ok(()))
                .into_future(),
        )
    }

    fn close_channel(&self, channel_id: ChannelId) -> Box<Future<Item = (), Error = Error>> {
        // This is the event we'll wait for that would mean our contract call got executed with at least one confirmation
        let data = encode_call(
            "closeChannel(bytes32)",
            &[
                // channel id
                Token::Bytes(channel_id.to_vec().into()),
            ],
        );
        // Broadcast a transaction on the network with data
        Box::new(
            CRYPTO
                .broadcast_transaction(Action::Call(data), Uint256::from(0))
                .and_then(|_tx| Ok(())),
        )
    }
    fn start_settling_period(
        &self,
        channel_id: ChannelId,
        signature: Signature,
    ) -> Box<Future<Item = (), Error = Error>> {
        // This is the event we'll wait for that would mean our contract call got executed with at least one confirmation
        let data = encode_call(
            "startSettlingPeriod(bytes32,bytes)",
            &[
                // channel id
                Token::Bytes(channel_id.to_vec().into()),
                signature.into_bytes().to_vec().into(),
            ],
        );
        // Broadcast a transaction on the network with data
        Box::new(
            CRYPTO
                .broadcast_transaction(Action::Call(data), Uint256::from(0))
                .and_then(|_tx| Ok(())),
        )
    }

    fn redraw(
        &self,
        channel_id: ChannelId,
        channel_nonce: Uint256,
        old_balance_a: Uint256,
        old_balance_b: Uint256,
        new_balance_a: Uint256,
        new_balance_b: Uint256,
        expiration: Uint256,
        sig_a: Signature,
        sig_b: Signature,
    ) -> Box<Future<Item = (), Error = Error>> {
        println!(
            "old_balance_a={} old_balance_b={} new_balance_a={} new_balance_b={}",
            old_balance_a, old_balance_b, new_balance_a, new_balance_b
        );
        // Broadcast a transaction on the network with data
        // assert!(address0 != address1, "Unable to open channel to yourself");

        // // Reorder addresses
        // let (address0, address1, balance0, balance1, signature0, signature1) =
        //     if address0 > address1 {
        //         (
        //             address1, address0, balance1, balance0, signature1, signature0,
        //         )
        //     } else {
        //         (
        //             address0, address1, balance0, balance1, signature0, signature1,
        //         )
        //     };
        // assert!(address0 < address1);

        // let addr0_bytes: [u8; 32] = {
        //     let mut data: [u8; 32] = Default::default();
        //     data[12..].copy_from_slice(&address0.as_bytes());
        //     data
        // };
        // let addr1_bytes: [u8; 32] = {
        //     let mut data: [u8; 32] = Default::default();
        //     data[12..].copy_from_slice(&address1.as_bytes());
        //     data
        // };
        let event = CRYPTO.wait_for_event(
            "ChannelReDrawn(bytes32)",
            Some(vec![channel_id.into()]),
            None,
        );

        let payload = encode_call(
            "reDraw(bytes32,uint256,uint256,uint256,uint256,uint256,uint256,bytes,bytes)",
            &[
                // channelId
                Token::Bytes(channel_id.to_vec().into()),
                // sequenceNumber
                channel_nonce.into(),
                // oldBalance0
                old_balance_a.into(),
                // oldBalance1
                old_balance_b.into(),
                // newBalance0
                new_balance_a.into(),
                // newBalance1
                new_balance_b.into(),
                // expiration
                expiration.into(),
                // signature0
                sig_a.into_bytes().to_vec().into(),
                // signature
                sig_b.into_bytes().to_vec().into(),
            ],
        );
        let call = CRYPTO.broadcast_transaction(Action::Call(payload), 0u64.into());

        Box::new(
            call.join(event)
                .and_then(|(_tx, response)| {
                    println!("response {:?}", response);
                    Ok(())
                }).into_future(),
        )
    }
}
