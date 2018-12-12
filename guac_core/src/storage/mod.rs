use channel_client::types::Counterparty;
use clarity::Address;
use failure::Error;

use futures;

use futures::Future;

use qutex::{FutureGuard, Guard, QrwLock, Qutex};
use std::collections::HashMap;

/// Storage contains a futures aware RwLock (QrwLock) which controls access to the inner data
/// This outer Rwlock should only be mutated very rarely, only to insert and remove counterparties
pub struct Data {
    inner: QrwLock<HashMap<Address, Qutex<Counterparty>>>,
}

impl Data {
    pub fn new() -> Data {
        Data {
            inner: QrwLock::new(HashMap::new()),
        }
    }
}

pub trait Storage {
    fn get_counterparty(
        &self,
        k: Address,
    ) -> Box<Future<Item = Guard<Counterparty>, Error = Error>>;
    fn new_counterparty(
        &self,
        k: Address,
        v: Counterparty,
    ) -> Box<Future<Item = (), Error = Error>>;
}

impl Storage for Data {
    fn get_counterparty(
        &self,
        k: Address,
    ) -> Box<Future<Item = Guard<Counterparty>, Error = Error>> {
        Box::new(
            self.inner
                .clone()
                .read()
                .from_err()
                .and_then(move |data| match data.get(&k) {
                    Some(v) => futures::future::ok(v.clone().lock()),
                    None => futures::future::err(format_err!("Counterparty not found")),
                })
                .and_then(|v: FutureGuard<Counterparty>| v.from_err().and_then(|v| Ok(v))),
        )
    }

    fn new_counterparty(
        &self,
        k: Address,
        v: Counterparty,
    ) -> Box<Future<Item = (), Error = Error>> {
        Box::new(
            self.inner
                .clone()
                .write()
                .from_err()
                .and_then(move |mut data| {
                    if !data.contains_key(&k) {
                        data.insert(k.clone(), Qutex::new(v.clone()));
                    } else {
                        bail!("Counterparty already exists");
                    }
                    Ok(())
                }),
        )
    }
}
