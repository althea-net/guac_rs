use channel_client::types::Counterparty;
use clarity::Address;
use failure::Error;

use futures;
use futures::future::join_all;
use futures::Future;

use crypto::CryptoService;
use CRYPTO;

use qutex::{FutureGuard, Guard, QrwLock, Qutex};
use std::collections::HashMap;

// use channel_client::ChannelManager;

// lazy_static! {
//     pub static ref STORAGE: Storage = Storage {
//         inner: QrwLock::new(Data::default())
//     };
// }

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

// #[derive(Default)]
// struct Data {
//     /// This stores a mapping from eth address to channel managers which manage the eth address
//     /// The ChannelManagers are wrapped in a futures aware Mutex (a Qutex) to achieve inner
//     /// mutability (the outer Data struct and this the outer RwLock does not have to be locked for
//     /// writing to mutate a single ChannelManager)
//     addr_to_counterparty: HashMap<Address, Qutex<ChannelManager>>,
//     // This stores a mapping from eth address to counterparty, with no fancy interior mutability
//     // for the counterparty, as the the frequency of mutations to the counterparty will be
//     // very low (comparable to the addition and deletions of channels, in which case the outer
//     // RwLock needs to be locked for writing anyways)
//     // addr_to_counterparty: HashMap<Address, Counterparty>,
// }

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
        assert!(k != CRYPTO.own_eth_addr());
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

/*
#[cfg(test)]
mod test {
    use super::*;
    use std::thread;

    #[test]
    fn single_threaded_storage() {
        STORAGE.reset();
        let fut = STORAGE.set_data(Bytes32([0; 32]), ChannelManager::default()).and_then(|_| {
            STORAGE.get_data(Bytes32([0; 32])).and_then(|data|{
                println!("{:?}", *data);
                Ok(())
            })
        });

        fut.wait().unwrap();
    }

    #[test]
    fn multi_threaded_storage() {
        STORAGE.reset();
        let thread_count = 100;
        let mut threads = Vec::with_capacity(thread_count);
        let start_val = 0;

        for i in 0..thread_count {
            let fut = STORAGE.set_data(Bytes32([i as u8; 32]), ChannelManager::default()).and_then(move |_| {
                STORAGE.get_data(Bytes32([i as u8; 32])).and_then(|data|{
                    println!("{:?}", *data);

                    move || {
                        data
                    };

                    Ok(())
                })
            });

            threads.push(thread::spawn(|| {
                fut.wait().unwrap();
            }));
        }

        for thread in threads {
            thread.join().unwrap();
        }
    }
}
*/
