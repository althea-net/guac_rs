use actix_web::client;
use actix_web::HttpMessage;
use failure::Error;
use futures::Future;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cell::RefCell;
use std::str;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpStream;
use transport_protocol::TransportProtocol;
use web3::jsonrpc::request::Request;
use web3::jsonrpc::response::Response;

pub trait Client {
    fn request_method<T: Serialize, R: 'static>(
        &self,
        method: &str,
        params: T,
    ) -> Box<Future<Item = R, Error = Error>>
    where
        for<'de> R: Deserialize<'de>;
}

pub struct HTTPClient {
    id_counter: Arc<Mutex<RefCell<u64>>>,
    url: String,
}

impl HTTPClient {
    pub fn new(url: &str) -> Self {
        Self {
            id_counter: Arc::new(Mutex::new(RefCell::new(0u64))),
            url: url.to_string(),
        }
    }

    fn next_id(&self) -> u64 {
        let counter = self.id_counter.clone();
        let counter = counter.lock().unwrap();
        let mut value = counter.borrow_mut();
        *value += 1;
        *value
    }
}

impl Client for HTTPClient {
    fn request_method<T: Serialize, R: 'static>(
        &self,
        method: &str,
        params: T,
    ) -> Box<Future<Item = R, Error = Error>>
    where
        for<'de> R: Deserialize<'de>,
    {
        let payload = Request::new(self.next_id(), method, params);
        Box::new(
            client::post(&self.url)
                .json(payload)
                .unwrap()
                .send()
                .timeout(Duration::from_millis(1000))
                .from_err()
                .and_then(|response| {
                    response
                        .json()
                        .from_err()
                        .and_then(move |res: Response<R>| {
                            res.data.into_result().map_err(move |e| {
                                format_err!("JSONRPC Error {}: {}", e.code, e.message)
                            })
                        })
                }),
        )
    }
}
