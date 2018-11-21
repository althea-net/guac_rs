use clarity::Signature;
use eth_client::EthClient;
use failure::Error;
use futures::future;
use futures::Future;
use num256::Uint256;
use payment_contract::PaymentContract;
use payment_manager::PaymentManager;
use transport_protocol::TransportFactory;
use transports::http::client_factory::HTTPTransportFactory;

/// This is the implementation for complete payment flow for
/// a guac contract that could be found in https://github.com/althea-mesh/guac.
///
/// This structure holds every information that it needs to make the payment
/// flow complete and that will include network to network.
struct Guac {
    contract: Box<PaymentContract>,
    transport_factory: Box<TransportFactory>,
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
    pub fn new(contract: Box<PaymentContract>, transport_factory: Box<TransportFactory>) -> Self {
        Self {
            contract,
            transport_factory,
        }
    }
}

impl Default for Guac {
    /// Creates Guac instance with default implementations
    /// of various traits that are valid especially for Guac
    /// contract
    fn default() -> Self {
        Self {
            contract: Box::new(EthClient::new()),
            transport_factory: Box::new(HTTPTransportFactory::new()),
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
    /// Propose a
    /// On a successful call it returns a signature signed by other party. Later this
    /// signature is combined with our signature signed, and sent to the contract
    ///
    /// * `remote` - A remote address in a format of "addr:port" t
    fn propose(
        &self,
        remote: &str,
        balance0: Uint256,
        balance1: Uint256,
    ) -> Box<Future<Item = Signature, Error = Error>> {
        Box::new(
            future::result(
                self.transport_factory
                    .create_transport_protocol(remote.to_string()),
            ).and_then(move |transport| {
                // TODO: This is dummy value to get futures together
                Ok(Signature::new(0u64.into(), 1u64.into(), 2u64.into()))
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use channel_client::types::{Channel, UpdateTx};
    use clarity::{Address, Signature};
    #[cfg(test)]
    use double::Mock;
    #[cfg(test)]
    use payment_contract::ChannelId;
    use std::cell::RefCell;
    #[cfg(test)]
    use std::rc::Rc;
    use std::sync::Arc;
    #[cfg(test)]
    use tokio::prelude::*;
    use transport_protocol::{TransportFactory, TransportProtocol};
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
    struct MockContract {
        // Rc here is used for the purpose of getting another reference to the same mock object.
        mock_deposit: Rc<Mock<(Uint256), Result<(), CloneableError>>>,
        mock_open_channel: Mock<(Address, Uint256, Uint256), (ChannelId)>,
        mock_join_channel: Mock<(ChannelId, Uint256), ()>,
        mock_update_channel: Mock<(ChannelId, Uint256, Uint256, Uint256, Signature, Signature), ()>,
        mock_start_challenge: Mock<(ChannelId), ()>,
        mock_close_channel: Mock<(ChannelId), ()>,
    }

    impl Default for MockContract {
        fn default() -> Self {
            MockContract {
                // Return a "default error" to signalize that a behaviour should be
                // modified in a test case.
                mock_deposit: Rc::new(Mock::new(Err(CloneableError::DefaultError))),
                mock_open_channel: Mock::default(),
                mock_join_channel: Mock::default(),
                mock_update_channel: Mock::default(),
                mock_start_challenge: Mock::default(),
                mock_close_channel: Mock::default(),
            }
        }
    }

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

    #[derive(Clone)]
    struct MockTransport {
        mock_send_proposal_request: Rc<Mock<(Channel), Result<bool, CloneableError>>>,
    }
    impl Default for MockTransport {
        fn default() -> Self {
            Self {
                mock_send_proposal_request: Rc::new(Mock::new(Err(CloneableError::DefaultError))),
            }
        }
    }

    impl TransportProtocol for MockTransport {
        /// Send a proposal to other party
        fn send_proposal_request(
            &self,
            channel: &Channel,
        ) -> Box<Future<Item = bool, Error = Error>> {
            Box::new(
                future::result(self.mock_send_proposal_request.call((channel.clone())))
                    .from_err()
                    .into_future(),
            )
        }
        /// Sends a channel created request
        fn send_channel_created_request(
            &self,
            channel: &Channel,
        ) -> Box<Future<Item = (), Error = Error>> {
            unimplemented!();
        }
        /// Send channel update
        fn send_channel_update(
            &self,
            update_tx: &UpdateTx,
        ) -> Box<Future<Item = UpdateTx, Error = Error>> {
            unimplemented!();
        }
        /// Send channel joined
        fn send_channel_joined(&self, channel: &Channel) -> Box<Future<Item = (), Error = Error>> {
            unimplemented!();
        }
    }
    // A factory that always returns the same instance for any given URL
    struct MockTransportFactory {
        mock_create_transport_protocol:
            Rc<Mock<(String), Result<RefCell<Box<MockTransport>>, CloneableError>>>,
    }
    impl Default for MockTransportFactory {
        fn default() -> Self {
            Self {
                mock_create_transport_protocol: Rc::new(Mock::new(Err(
                    CloneableError::DefaultError,
                ))),
            }
        }
    }
    impl TransportFactory for MockTransportFactory {
        fn create_transport_protocol(&self, url: String) -> Result<Box<TransportProtocol>, Error> {
            match self.mock_create_transport_protocol.call((url)) {
                Ok(transport) => Ok(transport.into_inner()),
                Err(e) => Err(e.into()),
            }
        }
    }

    #[test]
    fn deposit() {
        let contract = MockContract::default();
        // let transport = RefCell::new(MockTransport::default());
        let factory = MockTransportFactory::default();

        // Specify behaviour for deposit() contract call
        let mock_deposit = contract.mock_deposit.clone();
        mock_deposit.return_ok(());

        // Specify behaviour for transport
        let mock_create_transport_protocol = factory.mock_create_transport_protocol.clone();

        let guac = Guac::new(Box::new(contract), Box::new(factory));
        guac.deposit(123u64.into()).wait().unwrap();

        // Verify calls to the contract happened
        assert!(mock_deposit.has_calls_exactly(vec![Uint256::from(123u64)]));
    }

    #[test]
    fn propose() {
        let contract = MockContract::default();
        let transport = RefCell::new(Box::new(MockTransport::default()));
        let factory = MockTransportFactory::default();

        // Specify behaviour for deposit() contract call
        let mock_propose = transport.borrow().mock_send_proposal_request.clone();
        mock_propose.return_ok(true);

        // Specify behaviour for transport
        let mock_create_transport_protocol = factory.mock_create_transport_protocol.clone();
        mock_create_transport_protocol.use_closure(Box::new(move |_params| {
            // This will always return clones of the same transport instance.
            let instance = transport.clone();
            Ok(instance)
        }));

        let guac = Guac::new(Box::new(contract), Box::new(factory));
        guac.propose("42.42.42.42:4242", 100u64.into(), 0u64.into())
            .wait()
            .unwrap();

        // Verify calls to the contract happened
        assert!(
            mock_create_transport_protocol.has_calls_exactly(vec!["42.42.42.42:4242".to_string()])
        );
        // XXX: This fails because we don't know yet what channel is it
        assert!(mock_propose.called(), "Proposal not sent to other node!");
    }
}
