use actix_web::client;
use actix_web::HttpMessage;
use actix_web::{AsyncResponder, FutureResponse, HttpRequest, HttpResponse};
use bytes::Bytes;
use failure::Error;
use futures::future::Future;
// use futures::Future;
use jsonrpc::request::Request;
use jsonrpc::response::Response;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cell::RefCell;
use std::str;
use std::sync::{Arc, Mutex};
use tokio::net::TcpStream;
use types::TransactionResponse;

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
        T: std::fmt::Debug,
        R: std::fmt::Debug,
    {
        let payload = Request::new(self.next_id(), method, params);
        println!("req {:?}", payload);
        Box::new(
            client::post(&self.url)
                .json(payload)
                .unwrap()
                .send()
                .from_err()
                .and_then(|response| {
                    // println!("got resss {:#?}", response.body());
                    // response.body().from_err().and_then(|res: Bytes| {
                    //     let data: R = serde_json::from_slice(&res)?;
                    //     println!("got ressss {:#?}", res);
                    //     Ok(data)
                    // })

                    response
                        .json()
                        .from_err()
                        .and_then(move |res: Response<R>| {
                            println!("got res {:#?}", res);
                            let data = res.data.into_result();
                            data.map_err(move |e| {
                                format_err!("JSONRPC Error {}: {}", e.code, e.message)
                            })
                        })
                }),
        )
    }
}
