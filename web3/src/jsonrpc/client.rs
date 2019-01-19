use crate::jsonrpc::request::Request;
use crate::jsonrpc::response::Response;
use actix_web::client;
use actix_web::HttpMessage;
use failure::Error;
use futures::future::Future;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::str;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub trait Client {
    fn request_method<T: Serialize, R: 'static>(
        &self,
        method: &str,
        params: T,
    ) -> Box<Future<Item = R, Error = Error>>
    where
        for<'de> R: Deserialize<'de>,
        T: std::fmt::Debug,
        R: std::fmt::Debug;
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
        let counter = counter.lock().expect("id error");
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
        T: std::fmt::Debug,
        R: std::fmt::Debug,
    {
        let payload = Request::new(self.next_id(), method, params);
        trace!("web3 request {:?}", payload);
        Box::new(
            client::post(&self.url)
                .json(payload)
                .expect("json error")
                .send()
                .timeout(Duration::from_millis(1000))
                .from_err()
                .and_then(|response| {
                    response
                        .json()
                        .from_err()
                        .and_then(move |res: Response<R>| {
                            trace!("got web3 response {:#?}", res);
                            let data = res.data.into_result();
                            data.map_err(move |e| {
                                format_err!("JSONRPC Error {}: {}", e.code, e.message)
                            })
                        })
                }),
        )
    }
}
