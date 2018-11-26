use channel_client::types::UpdateTx;
use clarity::abi::encode_tokens;
use clarity::abi::{encode_call, Token};
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

pub fn create_update_tx(update: UpdateTx) -> Transaction {
    let channel_id: [u8; 32] = update.channel_id.into();
    let nonce: [u8; 32] = update.nonce.into();
    let balance_a: [u8; 32] = update.balance_a.into();
    let balance_b: [u8; 32] = update.balance_b.into();
    let signature_a = update.signature_a.unwrap().to_string();
    let signature_b = update.signature_b.unwrap().to_string();
    let data = encode_call(
        "updateState(bytes32,uint256,uint256,uint256,string,string)",
        &[
            // channelId
            Token::Bytes(channel_id.to_vec()),
            // nonce
            Token::Bytes(nonce.to_vec()),
            // balanceA
            Token::Bytes(balance_a.to_vec()),
            // balanceB
            Token::Bytes(balance_b.to_vec()),
            // SigA
            signature_a.as_str().into(),
            // SigB
            signature_b.as_str().into(),
        ],
    );

    Transaction {
        to: Address::default(),
        nonce: Uint256::from(42u32),
        // TODO: set this semi automatically
        gas_price: Uint256::from(3000u32),
        // TODO: find out how much gas this contract acutally takes
        gas_limit: Uint256::from(50_000u32),
        value: Uint256::from(0u32),
        data,
        signature: None,
    }.sign(&CRYPTO.secret(), None)
}

/// Creates a payload for "joinChannel" contract call.
///
/// * `channel_id` - A valid channel ID
pub fn create_join_channel_payload(channel_id: ChannelId) -> Vec<u8> {
    encode_call(
        "joinChannel(bytes32,uint256)",
        &[
            // id
            Token::Bytes(channel_id.to_vec().into()),
            // tokenAmount
            0u32.into(), // should use `msg.value` ^
        ],
    )
}

/// Create a data that should be signed with a private key
/// and the signed data should be passed as a Signature to
/// create_update_channel_payload.
pub fn create_signature_data(
    channel_id: ChannelId,
    nonce: Uint256,
    balance_a: Uint256,
    balance_b: Uint256,
) -> Vec<u8> {
    encode_tokens(&[
        Token::Bytes(channel_id.to_vec()),
        nonce.into(),
        balance_a.into(),
        balance_b.into(),
    ])
}

pub fn create_start_challenge_payload(channel_id: ChannelId) -> Vec<u8> {
    encode_call(
        "startChallenge(bytes32)",
        &[
            // channel id
            Token::Bytes(channel_id.to_vec().into()),
        ],
    )
}

pub fn create_close_channel_payload(channel_id: ChannelId) -> Vec<u8> {
    encode_call(
        // function closeChannel(bytes32 channelId) public {
        "closeChannel(bytes32)",
        &[
            // channel id
            Token::Bytes(channel_id.to_vec().into()),
        ],
    )
}

pub struct EthClient;

impl EthClient {
    pub fn new() -> Self {
        Self {}
    }
}

impl PaymentContract for EthClient {
    fn quick_deposit(&self, value: Uint256) -> Box<Future<Item = (), Error = Error>> {
        let payload = encode_call("quickDeposit()", &[]);
        let call = CRYPTO
            .broadcast_transaction(Action::Call(payload), value)
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
                    let mut data: [u8; 32] = Default::default();
                    ensure!(
                        response.data.0.len() == 32,
                        "Invalid data length in ChannelOpened event"
                    );
                    data.copy_from_slice(&response.data.0);
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

    fn start_challenge(&self, channel_id: ChannelId) -> Box<Future<Item = (), Error = Error>> {
        // This is the event we'll wait for that would mean our contract call got executed with at least one confirmation

        let event = CRYPTO.wait_for_event(
            "ChannelChallenge(bytes32,uint256,address)",
            Some(vec![channel_id.into()]),
            None,
        );

        // Broadcast a transaction on the network with data
        let call = CRYPTO.broadcast_transaction(
            Action::Call(create_start_challenge_payload(channel_id)),
            Uint256::from(0),
        );

        Box::new(
            call.join(event)
                .and_then(|(_tx, response)| ok(()))
                .into_future(),
        )
    }

    fn close_channel(&self, channel_id: ChannelId) -> Box<Future<Item = (), Error = Error>> {
        // This is the event we'll wait for that would mean our contract call got executed with at least one confirmation

        let event =
            CRYPTO.wait_for_event("ChannelClose(bytes32)", Some(vec![channel_id.into()]), None);

        // Broadcast a transaction on the network with data
        let call = CRYPTO.broadcast_transaction(
            Action::Call(create_close_channel_payload(channel_id)),
            Uint256::from(0),
        );

        Box::new(
            call.join(event)
                .and_then(|(_tx, _response)| ok(()))
                .into_future(),
        )
    }
}

#[test]
fn test_create_update_tx() {
    let tx = create_update_tx(UpdateTx {
        nonce: 0u32.into(),
        balance_a: 23u32.into(),
        balance_b: 23u32.into(),
        channel_id: 11u32.into(),
        signature_a: Some(Signature::new(1u32.into(), 2u32.into(), 3u32.into())),
        signature_b: Some(Signature::new(4u32.into(), 5u32.into(), 6u32.into())),
    });
    trace!("tx: {:?}", tx);
}

#[test]
fn test_join_channel_tx() {
    let data = create_join_channel_payload([0u8; 32]);
    assert!(data.len() > 0);
}
