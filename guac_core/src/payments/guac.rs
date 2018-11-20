use eth_client::EthClient;
use failure::Error;
use futures::Future;
use num256::Uint256;
use payment_contract::PaymentContract;
use payment_manager::PaymentManager;

/// This is the implementation for complete payment flow for
/// a guac contract that could be found in https://github.com/althea-mesh/guac.
///
/// This structure holds every information that it needs to make the payment
/// flow complete and that will include network to network.
struct Guac {
    contract: Box<PaymentContract>,
}

impl Guac {
    /// Creates new Guac instance with specified instances
    /// of the contract.
    ///
    /// You can use this method to inject mocked traits for tests,
    /// in production use `Guac::default()` which would use appropriate
    /// instances for production use.
    ///
    /// * `contract` - A boxed instance of a PaymentContract trait.
    pub fn new(contract: Box<PaymentContract>) -> Self {
        Self { contract }
    }
}

impl Default for Guac {
    /// Creates Guac instance with default implementations
    /// of various traits that are valid especially for Guac
    /// contract
    fn default() -> Self {
        Self {
            contract: Box::new(EthClient::new()),
        }
    }
}

impl PaymentManager for Guac {
    /// Deposit an amount of ETH in the Guac's contract address.
    ///
    /// Future is resolved once the transaction is successfuly broadcasted to the
    /// network.
    fn deposit(&self, value: Uint256) -> Box<Future<Item = (), Error = Error>> {
        self.contract.deposit(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity::{Address, Signature};
    #[cfg(test)]
    use double::Mock;
    #[cfg(test)]
    use payment_contract::ChannelId;
    #[cfg(test)]
    use std::sync::Arc;
    #[cfg(test)]
    use tokio::prelude::*;

    /// A cloneable error to use instead of failure::Error which isn't clonable,
    /// and double expects all values to be cloneable, and hashable.
    #[derive(Fail, Debug, Clone)]
    #[cfg(test)]
    enum CloneableError {
        #[fail(display = "This is default error")]
        DefaultError,
    }

    /// This contract implementation delegates calls to trait methods into a Mock objects.
    ///
    /// Mostly to overcome the fact that double expects results to be cloneable, but
    /// Futures are not.
    #[cfg(test)]
    struct MockContract {
        // Arc here is used for the purpose of getting another reference to the same mock object.
        mock_deposit: Arc<Mock<(Uint256), Result<(), CloneableError>>>,
        mock_open_channel: Mock<(Address, Uint256, Uint256), (ChannelId)>,
        mock_join_channel: Mock<(ChannelId, Uint256), ()>,
        mock_update_channel: Mock<(ChannelId, Uint256, Uint256, Uint256, Signature, Signature), ()>,
        mock_start_challenge: Mock<(ChannelId), ()>,
        mock_close_channel: Mock<(ChannelId), ()>,
    }

    #[cfg(test)]
    impl Default for MockContract {
        fn default() -> Self {
            MockContract {
                // Return a "default error" to signalize that a behaviour should be
                // modified in a test case.
                mock_deposit: Arc::new(Mock::new(Err(CloneableError::DefaultError))),
                mock_open_channel: Mock::default(),
                mock_join_channel: Mock::default(),
                mock_update_channel: Mock::default(),
                mock_start_challenge: Mock::default(),
                mock_close_channel: Mock::default(),
            }
        }
    }

    #[cfg(test)]
    impl PaymentContract for MockContract {
        fn deposit(&self, value: Uint256) -> Box<Future<Item = (), Error = Error>> {
            Box::new(
                future::result(self.mock_deposit.call((value)))
                    .from_err()
                    .into_future(),
            )
        }
        fn open_channel(
            &self,
            to: Address,
            challenge: Uint256,
            value: Uint256,
        ) -> Box<Future<Item = ChannelId, Error = Error>> {
            unimplemented!();
        }
        fn join_channel(
            &self,
            channel_id: ChannelId,
            value: Uint256,
        ) -> Box<Future<Item = (), Error = Error>> {
            unimplemented!();
        }
        fn update_channel(
            &self,
            channel_id: ChannelId,
            channel_nonce: Uint256,
            balance_a: Uint256,
            balance_b: Uint256,
            sig_a: Signature,
            sig_b: Signature,
        ) -> Box<Future<Item = (), Error = Error>> {
            unimplemented!();
        }
        fn start_challenge(&self, channel_id: ChannelId) -> Box<Future<Item = (), Error = Error>> {
            unimplemented!();
        }
        fn close_channel(&self, channel_id: ChannelId) -> Box<Future<Item = (), Error = Error>> {
            unimplemented!();
        }
    }

    #[test]
    fn deposit() {
        let contract = MockContract::default();

        // Specify behaviour for deposit() contract call
        let mock_deposit = contract.mock_deposit.clone();
        mock_deposit.return_ok(());

        let guac = Guac::new(Box::new(contract));
        guac.deposit(123u64.into()).wait().unwrap();

        // Verify calls to the contract happened
        assert!(mock_deposit.has_calls_exactly(vec![Uint256::from(123u64)]));
    }
}
