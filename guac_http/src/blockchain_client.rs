use clarity::abi::derive_signature;
use clarity::abi::{encode_call, encode_tokens};
use clarity::utils::bytes_to_hex_str;
use clarity::Transaction;
use clarity::{Address, PrivateKey, Signature};
use failure::Error;
use futures::IntoFuture;
use futures::Stream;
use futures::{future, Future};
use guac_core::channel_client::types::{NewChannelTx, ReDrawTx, UpdateTx};
use guac_core::BlockchainApi;
use num256::Uint256;
use web3::client::Web3;
use web3::client::Web3Client;
use web3::types::{Log, NewFilter};

fn bytes_to_data(s: &[u8]) -> String {
    let mut foo = "0x".to_string();
    foo.push_str(&bytes_to_hex_str(&s));
    foo
}

pub struct BlockchainClient {
    web3: Web3Client,
    contract_address: Address,
    own_address: Address,
    secret: PrivateKey,
}

impl BlockchainClient {
    fn wait_for_event(
        &self,
        event: &str,
        topic1: Option<Vec<[u8; 32]>>,
        topic2: Option<Vec<[u8; 32]>>,
    ) -> Box<Future<Item = Log, Error = Error>> {
        let web3 = self.web3.clone();
        // Build a filter with specified topics
        let mut new_filter = NewFilter::default();
        new_filter.address = vec![self.contract_address.clone()];
        new_filter.topics = Some(vec![
            Some(vec![Some(bytes_to_data(&derive_signature(event)))]),
            topic1.map(|v| v.into_iter().map(|val| Some(bytes_to_data(&val))).collect()),
            topic2.map(|v| v.into_iter().map(|val| Some(bytes_to_data(&val))).collect()),
        ]);
        Box::new(
            web3.eth_new_filter(new_filter)
                .and_then(move |filter_id| {
                    web3.eth_get_filter_changes(filter_id.clone())
                        .into_future()
                        .map(move |(head, _tail)| (filter_id, head))
                        .map_err(|(e, _)| e)
                })
                .and_then(|(filter_id, head)| {
                    web3.eth_uninstall_filter(filter_id).and_then(move |r| {
                        ensure!(r, "Unable to properly uninstall filter");
                        Ok(head)
                    })
                })
                .map(move |maybe_log| maybe_log.expect("Expected log data but None found"))
                .from_err()
                .into_future(),
        )
    }

    fn broadcast_transaction(&self, data: Vec<u8>) -> Box<Future<Item = Uint256, Error = Error>> {
        let web3 = self.web3.clone();
        let contract_address = self.contract_address.clone();
        let secret = self.secret.clone();

        let props = web3
            .eth_gas_price()
            .join(web3.eth_get_transaction_count(self.own_address));

        Box::new(
            props
                .and_then(move |(gas_price, nonce)| {
                    let transaction = Transaction {
                        to: contract_address,
                        nonce: nonce,
                        gas_price: gas_price.into(),
                        gas_limit: 6721975u32.into(),
                        value: 0u64.into(),
                        data: data,
                        signature: None,
                    };

                    let transaction = transaction.sign(&secret, Some(1u64));

                    web3.eth_send_raw_transaction(transaction.to_bytes().unwrap())
                    // .into_future()
                    // .map_err(GuacError::from)
                    // .and_then(|tx| ok(format!("0x{:x}", tx).parse().unwrap()))
                    // .from_err()
                })
                .into_future(),
        )
    }
}

impl BlockchainApi for BlockchainClient {
    fn new_channel(
        &self,
        new_channel_tx: &NewChannelTx,
    ) -> Box<Future<Item = Uint256, Error = Error>> {
        let addr_0_bytes: [u8; 32] = {
            let mut data: [u8; 32] = Default::default();
            data[12..].copy_from_slice(&new_channel_tx.address_0.as_bytes());
            data
        };
        let addr_1_bytes: [u8; 32] = {
            let mut data: [u8; 32] = Default::default();
            data[12..].copy_from_slice(&new_channel_tx.address_1.as_bytes());
            data
        };

        let event = self.wait_for_event(
            "ChannelOpened(address,address,bytes32)",
            Some(vec![addr_0_bytes]),
            Some(vec![addr_1_bytes]),
        );

        let payload = encode_call(
            "newChannel(address,address,uint256,uint256,uint256,uint256,bytes,bytes)",
            &[
                // address0
                new_channel_tx.address_0.into(),
                // address1
                new_channel_tx.address_1.into(),
                // balance0
                new_channel_tx.balance_0.into(),
                // balance1
                new_channel_tx.balance_1.into(),
                // expiration
                new_channel_tx.expiration.into(),
                // settlingPeriodLength in blocks
                new_channel_tx.settling_period_length.into(),
                // signature_0
                new_channel_tx
                    .signature_0
                    .expect("No signature_0 supplied")
                    .into_bytes()
                    .to_vec()
                    .into(),
                // signature_1
                new_channel_tx
                    .signature_1
                    .expect("No signature_1 supplied")
                    .into_bytes()
                    .to_vec()
                    .into(),
            ],
        );
        let call = self.broadcast_transaction(payload);

        Box::new(
            call.join(event)
                .and_then(|(_tx, response)| {
                    let mut data: [u8; 32] = Default::default();
                    ensure!(
                        response.data.len() == 32,
                        "Invalid data length in ChannelOpened event"
                    );
                    data.copy_from_slice(&response.data);
                    Ok(data.into())
                })
                .into_future(),
        )
    }

    fn re_draw(&self, new_channel: &ReDrawTx) -> Box<Future<Item = Uint256, Error = Error>> {}

    fn check_for_open(
        &self,
        address_0: &Address,
        address_1: &Address,
    ) -> Box<Future<Item = Uint256, Error = Error>> {

    }

    fn check_for_re_draw(
        &self,
        channel_id: &Uint256,
    ) -> Box<Future<Item = Uint256, Error = Error>> {

    }
}
