use std::sync::Mutex;
use web3;

#[derive(Debug, Fail)]
pub enum Error {
    // TODO: How to store web3::Error properly instead of string and to avoid weird threading errors?
    #[fail(display = "Web3 error: {}", _0)]
    Web3Error(String),
}
