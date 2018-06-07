use althea_types::Bytes32;
use failure::Error;
use futures::Future;
use qutex::{FutureWriteGuard, Guard, QrwLock, Qutex};
use std::collections::HashMap;

use channel_client::types::ChannelManager;

lazy_static! {
    pub static ref STORAGE: Storage = Storage {
        data: QrwLock::new(HashMap::new())
    };
}

pub struct Storage {
    data: QrwLock<HashMap<Bytes32, Qutex<ChannelManager>>>,
}

impl Storage {
    pub fn get_data(&self, k: Bytes32) -> impl Future<Item = Guard<ChannelManager>, Error = Error> {
        self.data
            .clone()
            .read()
            .and_then(move |data| data[&k].clone().lock())
            .from_err()
    }

    pub fn set_data(&self, k: Bytes32, v: ChannelManager) -> impl Future<Item = (), Error = Error> {
        self.data
            .clone()
            .write()
            .and_then(move |mut data| {
                data.insert(k, Qutex::new(v));
                Ok(())
            })
            .from_err()
    }

    pub fn reset(&self) {
        *self.data.clone().write().wait().unwrap() = HashMap::new()
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
