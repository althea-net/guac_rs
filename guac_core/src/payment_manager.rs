use failure::Error;
use futures::Future;
use num256::Uint256;

/// This is the main public facing trait that all libraries should depend on
/// to make payments.
///
/// Implementations of this trait should combine PaymentContract, and
/// TransportProtocol to implement a payment flow as described in diagrams
/// and specifications found in docs/ folder.
pub trait PaymentManager {
    /// Deposit an amount into the Wallet.
    ///
    /// This should returns a future that gets resolved once the implementation
    /// is sure that the transaction went through, and the user deposited
    /// provided amount into his wallet in contract.
    fn deposit(&self, value: Uint256) -> Box<Future<Item = (), Error = Error>>;
}
