use althea_types::EthAddress;
use counterparty::Counterparty;
use failure::Error;

use futures;
use futures::future::join_all;
use futures::Future;

use crypto::CryptoService;
use CRYPTO;

use qutex::{FutureGuard, Guard, QrwLock, Qutex};
use std::collections::HashMap;

use channel_client::ChannelManager;

lazy_static! {
    pub static ref STORAGE: Storage = Storage {
        inner: QrwLock::new(Data::default())
    };
}

/// Storage contains a futures aware RwLock (QrwLock) which controls access to the inner data
/// This outer Rwlock should only be mutated very rarely, only to insert and remove counterparties
pub struct Storage {
    inner: QrwLock<Data>,
}

#[derive(Default)]
struct Data {
    /// This stores a mapping from eth address to channel managers which manage the eth address
    /// The ChannelManagers are wrapped in a futures aware Mutex (a Qutex) to achieve inner
    /// mutability (the outer Data struct and this the outer RwLock does not have to be locked for
    /// writing to mutate a single ChannelManager)
    addr_to_channel: HashMap<EthAddress, Qutex<ChannelManager>>,
    /// This stores a mapping from eth address to counterparty, with no fancy interior mutability
    /// for the counterparty, as the the frequency of mutations to the counterparty will be
    /// very low (comparable to the addition and deletions of channels, in which case the outer
    /// RwLock needs to be locked for writing anyways)
    addr_to_counterparty: HashMap<EthAddress, Counterparty>,
}

impl Storage {
    pub fn get_all_counterparties(&self) -> impl Future<Item = Vec<Counterparty>, Error = Error> {
        self.inner
            .clone()
            .read()
            .and_then(|data| {
                let mut keys = Vec::new();
                for i in data.addr_to_counterparty.values() {
                    keys.push(i.clone());
                }
                Ok(keys)
            }).from_err()
    }

    pub fn get_all_channel_managers_mut(
        &self,
    ) -> impl Future<Item = Vec<Guard<ChannelManager>>, Error = Error> {
        self.inner
            .clone()
            .read()
            .and_then(|data| {
                let mut keys = Vec::new();
                for i in data.addr_to_channel.values() {
                    keys.push(i.clone().lock());
                }
                join_all(keys)
            }).from_err()
    }

    pub fn get_channel(
        &self,
        k: EthAddress,
    ) -> impl Future<Item = Guard<ChannelManager>, Error = Error> {
        self.inner
            .clone()
            .read()
            .from_err()
            .and_then(move |data| match data.addr_to_channel.get(&k) {
                Some(v) => futures::future::ok(v.clone().lock()),
                None => futures::future::err(format_err!("node not found")),
            }).and_then(|v: FutureGuard<ChannelManager>| v.from_err().and_then(|v| Ok(v)))
    }

    pub fn init_data(
        &self,
        k: Counterparty,
        v: ChannelManager,
    ) -> impl Future<Item = (), Error = Error> {
        assert!(k.address != CRYPTO.own_eth_addr());
        self.inner
            .clone()
            .write()
            .from_err()
            .and_then(move |mut data| {
                if !data.addr_to_counterparty.contains_key(&k.address) {
                    data.addr_to_counterparty.insert(k.address, k.clone());
                    data.addr_to_channel.insert(k.address, Qutex::new(v));
                } else {
                    bail!("Already exists");
                }
                Ok(())
            })
    }

    pub fn reset(&self) {
        *self.inner.clone().write().wait().unwrap() = Data::default()
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
