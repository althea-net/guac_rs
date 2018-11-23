use clarity::{Address, Signature};
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
    /// Withdraws an amount from the Wallet
    ///
    /// This returns a future which is resolved once the blockchain transaction
    /// went through. It will resolve to an error when the provided value is
    /// incorrect, or the transaction couldn't be sent properly to the network.
    ///
    /// Requires an amount of "value" to be in the wallet inside the contract.
    fn withdraw(&self, value: Uint256) -> Box<Future<Item = (), Error = Error>>;
    /// Registers new counterparty for a given parameters.
    fn register_counterparty(
        &self,
        remote: &str,
        address0: Address,
        address1: Address,
        balance0: Uint256,
        balance1: Uint256,
    ) -> Box<Future<Item = (), Error = Error>>;
    ///
    /// Proposes a channel to the other party.
    ///
    /// This method should resolve future with a valid signature of the other party.
    ///
    /// * `remote` - Remote address
    /// * `balance0` - Our proposed amount
    /// * `balance1` - Other proposed amount
    fn propose(&self, channel_id: Uint256) -> Box<Future<Item = Signature, Error = Error>>;

    /// Created new channel on the network and sends notification to other party.
    fn new_channel(
        &self,
        channel_id: Uint256,
        signature: Signature,
    ) -> Box<Future<Item = (), Error = Error>>;
}
