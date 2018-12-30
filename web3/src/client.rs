//! Byte-order safe and lightweight Web3 client.
//!
//! Rust-web3 has its problems because it uses ethereum-types which does not
//! work on big endian. We can do better than that just crafting our own
//! JSONRPC requests.
//!
use crate::jsonrpc::client::{Client, HTTPClient};
use crate::types::{Log, NewFilter, TransactionRequest, TransactionResponse};
use clarity::utils::bytes_to_hex_str;
use clarity::Address;
use failure::Error;
use futures::stream;
use futures::IntoFuture;
use futures::{Future, Stream};
use futures_timer::Interval;
use num256::Uint256;
use std::sync::Arc;
use std::time::Duration;

/// An instance of Web3Client.
#[derive(Clone)]
pub struct Web3 {
    jsonrpc_client: Arc<Box<HTTPClient>>,
}

impl Web3 {
    pub fn new(url: &str) -> Self {
        Self {
            jsonrpc_client: Arc::new(Box::new(HTTPClient::new(url))),
        }
    }

    pub fn eth_accounts(&self) -> Box<Future<Item = Vec<Address>, Error = Error>> {
        self.jsonrpc_client
            .request_method("eth_accounts", Vec::<String>::new())
    }
    pub fn net_version(&self) -> Box<Future<Item = String, Error = Error>> {
        self.jsonrpc_client
            .request_method("net_version", Vec::<String>::new())
    }
    pub fn eth_new_filter(
        &self,
        new_filter: NewFilter,
    ) -> Box<Future<Item = Uint256, Error = Error>> {
        self.jsonrpc_client
            .request_method("eth_newFilter", vec![new_filter])
    }
    pub fn eth_uninstall_filter(&self, filter: Uint256) -> Box<Future<Item = bool, Error = Error>> {
        self.jsonrpc_client
            .request_method("eth_uninstallFilter", vec![filter])
    }
    pub fn eth_get_filter_changes(
        &self,
        filter: Uint256,
    ) -> Box<Stream<Item = Log, Error = Error>> {
        let jsonrpc_client = self.jsonrpc_client.clone();
        Box::new(
            // Every 1 second
            Interval::new(Duration::from_secs(1))
                .map(move |()| {
                    jsonrpc_client
                        .clone()
                        // Call eth_getFilterChanges every second
                        .request_method("eth_getFilterChanges", vec![filter.clone()])
                        // Convert list of logs into a Future of Stream
                        .map(move |logs: Vec<Log>| stream::iter_ok(logs.into_iter()))
                        .into_future()
                        // Flatten future of stream into a stream therefore extracting nested stream of logs
                        .flatten_stream()
                })
                // Flatten stream of streams into a single stream
                .flatten(),
        )
    }

    pub fn eth_get_transaction_count(
        &self,
        address: Address,
    ) -> Box<Future<Item = Uint256, Error = Error>> {
        self.jsonrpc_client.request_method(
            "eth_getTransactionCount",
            vec![address.to_string(), "latest".to_string()],
        )
    }
    pub fn eth_gas_price(&self) -> Box<Future<Item = Uint256, Error = Error>> {
        self.jsonrpc_client
            .request_method("eth_gasPrice", Vec::<String>::new())
    }
    pub fn eth_get_balance(&self, address: Address) -> Box<Future<Item = Uint256, Error = Error>> {
        self.jsonrpc_client.request_method(
            "eth_getBalance",
            vec![address.to_string(), "latest".to_string()],
        )
    }
    pub fn eth_send_transaction(
        &self,
        transactions: Vec<TransactionRequest>,
    ) -> Box<Future<Item = Uint256, Error = Error>> {
        self.jsonrpc_client
            .request_method("eth_sendTransaction", transactions)
    }
    pub fn eth_block_number(&self) -> Box<Future<Item = Uint256, Error = Error>> {
        self.jsonrpc_client
            .request_method("eth_blockNumber", Vec::<String>::new())
    }
    pub fn eth_send_raw_transaction(
        &self,
        data: Vec<u8>,
    ) -> Box<Future<Item = Uint256, Error = Error>> {
        self.jsonrpc_client.request_method(
            "eth_sendRawTransaction",
            vec![format!("0x{}", bytes_to_hex_str(&data))],
        )
    }
    pub fn eth_get_transaction_by_hash(
        &self,
        hash: Uint256,
    ) -> Box<Future<Item = Option<TransactionResponse>, Error = Error>> {
        self.jsonrpc_client.request_method(
            "eth_getTransactionByHash",
            /// XXX: Technically it doesn't need to be Uint256, but since send_raw_transaction is
            /// returning it we'll keep it consistent.
            vec![format!("{:#066x}", hash)],
        )
    }
    pub fn evm_snapshot(&self) -> Box<Future<Item = Uint256, Error = Error>> {
        self.jsonrpc_client
            .request_method("evm_snapshot", Vec::<String>::new())
    }
    pub fn evm_revert(&self, snapshot_id: Uint256) -> Box<Future<Item = Uint256, Error = Error>> {
        self.jsonrpc_client
            .request_method("evm_revert", vec![format!("{:#066x}", snapshot_id)])
    }
}
