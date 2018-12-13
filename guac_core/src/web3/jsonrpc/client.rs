use actix_web::client;
use actix_web::client::Connection;
use actix_web::HttpMessage;
use failure::Error;
use futures::{future, Future};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::net::ToSocketAddrs;
use std::str;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpStream as TokioTcpStream;
use web3::jsonrpc::request::Request;
use web3::jsonrpc::response::Response;

pub trait Client {
    fn request_method<T: Serialize + 'static, R: 'static>(
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
    fn request_method<T: 'static + Serialize, R: 'static>(
        &self,
        method: &str,
        params: T,
    ) -> Box<Future<Item = R, Error = Error>>
    where
        for<'de> R: Deserialize<'de>,
    {
        let payload = Request::new(self.next_id(), method, params);
        trace!("Trying to parse {}", self.url);
        let sanitized_url = self
            .url
            .replace("http://", "")
            .replace("https://", "")
            .replace("/", "");
        match sanitized_url.to_socket_addrs() {
            Ok(mut socket_iter) => match socket_iter.next() {
                Some(socket) => {
                    trace!("Got socket, making web3 request");
                    let stream = TokioTcpStream::connect(&socket);
                    let url = self.url.clone();

                    Box::new(stream.from_err().and_then(move |open_stream| {
                        trace!(
                            "tokio tcp stream connected for web3 call!, posting to {}",
                            url
                        );
                        client::post(&url)
                            .with_connection(Connection::from_stream(open_stream))
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
                            })
                    }))
                }
                None => Box::new(future::err(format_err!(
                    "No entry in socketaddr's list for web3 lookup"
                ))),
            },
            Err(e) => Box::new(future::err(e.into())),
        }
    }
}
