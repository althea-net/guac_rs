use error::GuacError;
use failure::Error;
use std::ops::Deref;
use web3::transports::{EventLoopHandle, Http};
use web3::Web3;
/// A handle that contains event loop instance and a web3 instance
///
/// EventLoop has to live at least as long as the "Web3" object, or
/// otherwise calls will fail. We achieve this by implementing a Deref
/// trait that would return a borrowed Web3 object.
pub struct Web3Handle(EventLoopHandle, Web3<Http>);

impl Deref for Web3Handle {
    type Target = Web3<Http>;
    fn deref(&self) -> &Web3<Http> {
        &self.1
    }
}

impl Web3Handle {
    pub fn new(address: &str) -> Result<Web3Handle, Error> {
        let (evloop, transport) = Http::new(&address).map_err(GuacError::from)?;
        Ok(Web3Handle(evloop, Web3::new(transport)))
    }
}
