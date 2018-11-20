use failure::Error;
use futures::Future;
use num256::Uint256;
use payment_manager::PaymentManager;

/// This is the implementation for complete payment flow for
/// a guac contract that could be found in https://github.com/althea-mesh/guac.
///
/// This structure holds every information that it needs to make the payment
/// flow complete and that will include network to network.
struct Guac;

impl PaymentManager for Guac {
    /// Deposit an amount of ETH in the Guac's contract address.
    ///
    /// Future is resolved once the transaction is successfuly broadcasted to the
    /// network.
    fn deposit(&self, _value: Uint256) -> Box<Future<Item = (), Error = Error>> {
        unimplemented!();
    }
}

#[test]
fn deposit() {
    let _guac = Guac {};
}
