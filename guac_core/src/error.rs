use std::sync::Mutex;

#[derive(Debug, Fail)]
pub enum GuacError {
    // TODO: How to store web3::Error properly instead of string and to avoid weird threading errors?
    #[fail(display = "Web3 error: {}", _0)]
    Web3Error(String),
}
