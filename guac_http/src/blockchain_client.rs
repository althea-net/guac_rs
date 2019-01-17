use clarity::abi::derive_signature;
use clarity::abi::{encode_call, Token};
use clarity::utils::bytes_to_hex_str;
use clarity::Transaction;
use clarity::{Address, PrivateKey};
use failure::Error;
use futures::Future;
use futures::IntoFuture;
use futures::Stream;
use guac_core::types::{NewChannelTx, ReDrawTx};
use guac_core::BlockchainApi;
use num256::Uint256;
use web3::client::Web3;
use web3::types::{Data, Log, NewFilter, TransactionRequest};

fn bytes_to_data(s: &[u8]) -> String {
    let mut foo = "0x".to_string();
    foo.push_str(&bytes_to_hex_str(&s));
    foo
}

pub enum Action {
    /// Sends a "traditional" ETH transfer
    To(Address),
    /// Does a contract call with provided ddata
    Call(Vec<u8>),
}

pub struct BlockchainClient {
    web3: Web3,
    contract_address: Address,
    own_address: Address,
    secret: PrivateKey,
}

impl BlockchainClient {
    pub fn new(
        contract_address: Address,
        own_address: Address,
        secret: PrivateKey,
        full_node_url: &String,
    ) -> BlockchainClient {
        BlockchainClient {
            contract_address,
            own_address,
            secret,
            web3: Web3::new(full_node_url),
        }
    }
    fn wait_for_event(
        &self,
        event: &str,
        topic1: Option<Vec<[u8; 32]>>,
        topic2: Option<Vec<[u8; 32]>>,
    ) -> Box<Future<Item = Log, Error = Error>> {
        self.get_event(event, topic1, topic2, None, None)
    }

    fn check_for_event(
        &self,
        event: &str,
        topic1: Option<Vec<[u8; 32]>>,
        topic2: Option<Vec<[u8; 32]>>,
    ) -> Box<Future<Item = Option<Log>, Error = Error>> {
        let web3 = self.web3.clone();

        // Build a filter with specified topics
        let mut new_filter = NewFilter::default();
        new_filter.address = vec![self.contract_address.clone()];
        new_filter.topics = Some(vec![
            Some(vec![Some(bytes_to_data(&derive_signature(event)))]),
            topic1.map(|v| v.into_iter().map(|val| Some(bytes_to_data(&val))).collect()),
            topic2.map(|v| v.into_iter().map(|val| Some(bytes_to_data(&val))).collect()),
        ]);

        Box::new(web3.eth_get_logs(new_filter).and_then(|logs| {
            // Assuming the latest log is at the head of the vec
            Ok(logs.first().map(|log| log.clone()))
        }))
    }

    fn get_event(
        &self,
        event: &str,
        topic1: Option<Vec<[u8; 32]>>,
        topic2: Option<Vec<[u8; 32]>>,
        from_block: Option<String>,
        to_block: Option<String>,
    ) -> Box<Future<Item = Log, Error = Error>> {
        let web3 = self.web3.clone();
        // Build a filter with specified topics
        let mut new_filter = NewFilter::default();
        new_filter.address = vec![self.contract_address.clone()];
        new_filter.from_block = from_block;
        new_filter.to_block = to_block;
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
                        .and_then(move |(filter_id, head)| {
                            web3.eth_uninstall_filter(filter_id).and_then(move |r| {
                                ensure!(r, "Unable to properly uninstall filter");
                                Ok(head)
                            })
                        })
                })
                .map(move |maybe_log| maybe_log.expect("Expected log data but None found"))
                .from_err()
                .into_future(),
        )
    }

    fn send_raw_transaction(
        &self,
        to_address: Address,
        data: Vec<u8>,
        value: Uint256,
    ) -> Box<Future<Item = Uint256, Error = Error>> {
        let web3 = self.web3.clone();
        let secret = self.secret.clone();

        let props = web3
            .eth_gas_price()
            .join(web3.eth_get_transaction_count(self.own_address));

        Box::new(
            props
                .and_then(move |(gas_price, nonce)| {
                    let transaction = Transaction {
                        to: to_address,
                        nonce: nonce,
                        gas_price: gas_price.into(),
                        gas_limit: 6721975u32.into(),
                        value,
                        data,
                        signature: None,
                    };

                    let transaction = transaction.sign(&secret, Some(1u64));

                    web3.eth_send_raw_transaction(transaction.to_bytes().unwrap())
                })
                .into_future(),
        )
    }
}

impl BlockchainApi for BlockchainClient {
    fn balance_of(&self) -> Box<Future<Item = Uint256, Error = Error>> {
        let web3 = self.web3.clone();
        let contract_address = self.contract_address.clone();
        let own_address = self.own_address.clone();

        let props = web3
            .eth_gas_price()
            .join(web3.eth_get_transaction_count(own_address));

        let payload = encode_call("balanceOf(address)", &[own_address.into()]);

        Box::new(
            props
                .and_then(move |(gas_price, nonce)| {
                    let transaction = TransactionRequest {
                        from: own_address,
                        to: Some(contract_address),
                        nonce: Some(nonce),
                        gas: None,
                        gas_price: gas_price.into(),
                        value: Some(0u64.into()),
                        data: Some(Data(payload)),
                    };

                    web3.eth_call(transaction)
                })
                .and_then(|bytes| Ok(Uint256::from_bytes_be(&bytes))),
        )
    }

    fn deposit_then_new_channel(
        &self,
        amount: Uint256,
        new_channel_tx: NewChannelTx,
    ) -> Box<Future<Item = [u8; 32], Error = Error>> {
        let contract_address = self.contract_address.clone();
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
            "depositThenNewChannel(address,address,uint256,uint256,uint256,uint256,bytes,bytes)",
            &[
                new_channel_tx.address_0.into(),
                new_channel_tx.address_1.into(),
                new_channel_tx.balance_0.into(),
                new_channel_tx.balance_1.into(),
                new_channel_tx.expiration.into(),
                new_channel_tx.settling_period_length.into(),
                new_channel_tx
                    .signature_0
                    .expect("No signature_0 supplied")
                    .into_bytes()
                    .to_vec()
                    .into(),
                new_channel_tx
                    .signature_1
                    .expect("No signature_1 supplied")
                    .into_bytes()
                    .to_vec()
                    .into(),
            ],
        );

        let call = self.send_raw_transaction(contract_address, payload, amount);

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

    fn deposit_then_re_draw(
        &self,
        amount: Uint256,
        re_draw_tx: ReDrawTx,
    ) -> Box<Future<Item = (), Error = Error>> {
        let contract_address = self.contract_address.clone();
        let event = self.wait_for_event(
            "ChannelReDrawn(bytes32)",
            Some(vec![re_draw_tx.channel_id.into()]),
            None,
        );

        let payload = encode_call(
            "depositThenRedraw(bytes32,uint256,uint256,uint256,uint256,uint256,uint256,bytes,bytes)",
            &[
                Token::Bytes(re_draw_tx.channel_id.to_vec()),
                re_draw_tx.sequence_number.into(),
                re_draw_tx.old_balance_0.into(),
                re_draw_tx.old_balance_1.into(),
                re_draw_tx.new_balance_0.into(),
                re_draw_tx.new_balance_1.into(),
                re_draw_tx.expiration.into(),
                re_draw_tx
                    .signature_0
                    .expect("No signature_0 supplied")
                    .into_bytes()
                    .to_vec()
                    .into(),
                re_draw_tx
                    .signature_1
                    .expect("No signature_1 supplied")
                    .into_bytes()
                    .to_vec()
                    .into(),
            ],
        );

        let call = self.send_raw_transaction(contract_address, payload, amount);

        Box::new(
            call.join(event)
                .and_then(|(_tx, _response)| Ok(()))
                .into_future(),
        )
    }

    fn re_draw_then_withdraw(
        &self,
        amount: Uint256,
        re_draw_tx: ReDrawTx,
    ) -> Box<Future<Item = (), Error = Error>> {
        let contract_address = self.contract_address.clone();
        let event = self.wait_for_event(
            "ChannelReDrawn(bytes32)",
            Some(vec![re_draw_tx.channel_id.into()]),
            None,
        );

        println!("amount: {:?}, old_balance_0: {:?}, old_balance_1: {:?}, new_balance_0: {:?}, new_balance_1: {:?}", amount.clone(), re_draw_tx.old_balance_0.clone(), re_draw_tx.old_balance_1.clone(), re_draw_tx.new_balance_0.clone(), re_draw_tx.new_balance_1.clone());

        let payload = encode_call(
            "redrawThenWithdraw(uint256,bytes32,uint256,uint256,uint256,uint256,uint256,uint256,bytes,bytes)",
            &[
                amount.clone().into(),
                Token::Bytes(re_draw_tx.channel_id.to_vec()),
                re_draw_tx.sequence_number.into(),
                re_draw_tx.old_balance_0.into(),
                re_draw_tx.old_balance_1.into(),
                re_draw_tx.new_balance_0.into(),
                re_draw_tx.new_balance_1.into(),
                re_draw_tx.expiration.into(),
                re_draw_tx
                    .signature_0
                    .expect("No signature_0 supplied")
                    .into_bytes()
                    .to_vec()
                    .into(),
                re_draw_tx
                    .signature_1
                    .expect("No signature_1 supplied")
                    .into_bytes()
                    .to_vec()
                    .into(),
            ],
        );

        let call = self.send_raw_transaction(contract_address, payload, amount);

        Box::new(
            call.join(event)
                .and_then(|(_tx, _response)| Ok(()))
                .into_future(),
        )
    }

    fn check_for_open(
        &self,
        address_0: &Address,
        address_1: &Address,
    ) -> Box<Future<Item = Option<[u8; 32]>, Error = Error>> {
        let addr_0_bytes: [u8; 32] = {
            let mut data: [u8; 32] = Default::default();
            data[12..].copy_from_slice(&address_0.as_bytes());
            data
        };
        let addr_1_bytes: [u8; 32] = {
            let mut data: [u8; 32] = Default::default();
            data[12..].copy_from_slice(&address_1.as_bytes());
            data
        };
        Box::new(
            self.check_for_event(
                "ChannelOpened(address,address,bytes32)",
                Some(vec![addr_0_bytes]),
                Some(vec![addr_1_bytes]),
            )
            .and_then(|res| {
                if let Some(response) = res {
                    let mut data: [u8; 32] = Default::default();
                    ensure!(
                        response.data.len() == 32,
                        "Invalid data length in ChannelOpened event"
                    );
                    data.copy_from_slice(&response.data);
                    Ok(Some(data.into()))
                } else {
                    Ok(None)
                }
            }),
        )
    }

    fn check_for_re_draw(&self, channel_id: [u8; 32]) -> Box<Future<Item = (), Error = Error>> {
        Box::new(
            self.check_for_event(
                "ChannelReDrawn(bytes32)",
                Some(vec![channel_id.into()]),
                None,
            )
            .and_then(|_| Ok(())),
        )
    }

    fn quick_deposit(&self, value: Uint256) -> Box<Future<Item = (), Error = Error>> {
        let contract_address = self.contract_address.clone();
        let payload = encode_call("quickDeposit()", &[]);
        let call = self
            .send_raw_transaction(contract_address, payload, value)
            .map(|_| ());
        Box::new(call)
    }

    fn get_current_block(&self) -> Box<Future<Item = Uint256, Error = Error>> {
        self.web3.eth_block_number()
    }
}
