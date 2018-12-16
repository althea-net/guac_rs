//! Byte-order safe and lightweight Web3 client.
//!
//! Rust-web3 has its problems because it uses ethereum-types which does not
//! work on big endian. We can do better than that just crafting our own
//! JSONRPC requests.
//!
use clarity::utils::bytes_to_hex_str;
use clarity::Address;
use failure::Error;
use futures::prelude::*;
use futures::IntoFuture;
use futures::{future, stream};
use futures::{Future, Stream};
use futures_timer::Interval;
use jsonrpc::client::{Client, HTTPClient};
use num256::Uint256;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use types::{Log, NewFilter, TransactionRequest, TransactionResponse};

/// Trait that exposes common Web3 JSONRPC APIs in an asynchronous way
pub trait Web3 {
    /// Returns a list of addresses owned by client
    fn eth_accounts(&self) -> Box<Future<Item = Vec<Address>, Error = Error>>;
    fn net_version(&self) -> Box<Future<Item = String, Error = Error>>;
    fn eth_new_filter(&self, new_filter: NewFilter) -> Box<Future<Item = Uint256, Error = Error>>;
    fn eth_uninstall_filter(&self, filter: Uint256) -> Box<Future<Item = bool, Error = Error>>;
    fn eth_get_filter_changes(&self, filters: Uint256) -> Box<Stream<Item = Log, Error = Error>>;
    fn eth_get_transaction_count(
        &self,
        address: Address,
    ) -> Box<Future<Item = Uint256, Error = Error>>;

    fn eth_gas_price(&self) -> Box<Future<Item = Uint256, Error = Error>>;
    fn eth_get_balance(&self, address: Address) -> Box<Future<Item = Uint256, Error = Error>>;
    fn eth_send_transaction(
        &self,
        transactions: Vec<TransactionRequest>,
    ) -> Box<Future<Item = Uint256, Error = Error>>;
    fn eth_block_number(&self) -> Box<Future<Item = Uint256, Error = Error>>;
    fn eth_send_raw_transaction(&self, data: Vec<u8>)
        -> Box<Future<Item = Uint256, Error = Error>>;
    fn eth_get_transaction_by_hash(
        &self,
        hash: Uint256,
    ) -> Box<Future<Item = Option<TransactionResponse>, Error = Error>>;
}

/// An instance of Web3Client.
#[derive(Clone)]
pub struct Web3Client {
    jsonrpc_client: Arc<Box<HTTPClient>>,
}

impl Web3Client {
    pub fn new(url: &str) -> Self {
        Self {
            jsonrpc_client: Arc::new(Box::new(HTTPClient::new(url))),
        }
    }
}

impl Web3 for Web3Client {
    fn eth_accounts(&self) -> Box<Future<Item = Vec<Address>, Error = Error>> {
        self.jsonrpc_client
            .request_method("eth_accounts", Vec::<String>::new())
    }
    fn net_version(&self) -> Box<Future<Item = String, Error = Error>> {
        self.jsonrpc_client
            .request_method("net_version", Vec::<String>::new())
    }
    fn eth_new_filter(&self, new_filter: NewFilter) -> Box<Future<Item = Uint256, Error = Error>> {
        self.jsonrpc_client
            .request_method("eth_newFilter", vec![new_filter])
    }
    fn eth_uninstall_filter(&self, filter: Uint256) -> Box<Future<Item = bool, Error = Error>> {
        self.jsonrpc_client
            .request_method("eth_uninstallFilter", vec![filter])
    }
    fn eth_get_filter_changes(&self, filter: Uint256) -> Box<Stream<Item = Log, Error = Error>> {
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

    fn eth_get_transaction_count(
        &self,
        address: Address,
    ) -> Box<Future<Item = Uint256, Error = Error>> {
        self.jsonrpc_client.request_method(
            "eth_getTransactionCount",
            vec![address.to_string(), "latest".to_string()],
        )
    }
    fn eth_gas_price(&self) -> Box<Future<Item = Uint256, Error = Error>> {
        self.jsonrpc_client
            .request_method("eth_gasPrice", Vec::<String>::new())
    }
    fn eth_get_balance(&self, address: Address) -> Box<Future<Item = Uint256, Error = Error>> {
        self.jsonrpc_client.request_method(
            "eth_getBalance",
            vec![address.to_string(), "latest".to_string()],
        )
    }
    fn eth_send_transaction(
        &self,
        transactions: Vec<TransactionRequest>,
    ) -> Box<Future<Item = Uint256, Error = Error>> {
        self.jsonrpc_client
            .request_method("eth_sendTransaction", transactions)
    }
    fn eth_block_number(&self) -> Box<Future<Item = Uint256, Error = Error>> {
        self.jsonrpc_client
            .request_method("eth_blockNumber", Vec::<String>::new())
    }
    fn eth_send_raw_transaction(
        &self,
        data: Vec<u8>,
    ) -> Box<Future<Item = Uint256, Error = Error>> {
        self.jsonrpc_client.request_method(
            "eth_sendRawTransaction",
            vec![format!("0x{}", bytes_to_hex_str(&data))],
        )
    }
    fn eth_get_transaction_by_hash(
        &self,
        hash: Uint256,
    ) -> Box<Future<Item = Option<TransactionResponse>, Error = Error>> {
        self.jsonrpc_client.request_method(
            "eth_getTransactionByHash",
            /// XXX: Technically it doesn't need to be Uint256, but since send_raw_transaction is
            /// returning it we'll keep it consistent.
            vec![hash],
        )
    }
}
