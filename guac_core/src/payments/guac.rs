use channel_client::types::ChannelState;
use channel_storage::ChannelStorage;
use clarity::{Address, Signature};
use eth_client::EthClient;
use failure::Error;
use futures::future;
use futures::Future;
use num256::Uint256;
use payment_contract::PaymentContract;
use payment_manager::PaymentManager;
use std::sync::Arc;
use storage::in_memory::InMemoryStorage;
use transport_protocol::TransportFactory;
use transports::http::client_factory::HTTPTransportFactory;

/// This is the implementation for complete payment flow for
/// a guac contract that could be found in https://github.com/althea-mesh/guac.
///
/// This structure holds every information that it needs to make the payment
/// flow complete and that will include network to network.
struct Guac {
    contract: Arc<Box<PaymentContract>>,
    transport_factory: Arc<Box<TransportFactory>>,
    storage: Arc<Box<ChannelStorage>>,
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
    pub fn new(
        contract: Box<PaymentContract>,
        transport_factory: Box<TransportFactory>,
        storage: Box<ChannelStorage>,
    ) -> Self {
        Self {
            contract: Arc::new(contract),
            transport_factory: Arc::new(transport_factory),
            storage: Arc::new(storage),
        }
    }
}

impl Default for Guac {
    /// Creates Guac instance with default implementations
    /// of various traits that are valid especially for Guac
    /// contract
    fn default() -> Self {
        Self {
            contract: Arc::new(Box::new(EthClient::new())),
            transport_factory: Arc::new(Box::new(HTTPTransportFactory::new())),
            storage: Arc::new(Box::new(InMemoryStorage::new())),
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
    /// Withdraw an amount of ETH from the Guac contract.
    fn withdraw(&self, value: Uint256) -> Box<Future<Item = (), Error = Error>> {
        self.contract.withdraw(value)
    }
    /// Register a counterparty
    fn register_counterparty(
        &self,
        remote: &str,
        address0: Address,
        address1: Address,
        balance0: Uint256,
        balance1: Uint256,
    ) -> Box<Future<Item = (), Error = Error>> {
        Box::new(
            self.storage
                .register_channel(remote.to_string(), address0, address1, balance0, balance1)
                .and_then(|_channel| Ok(())),
        )
    }
    /// Propose a channel
    ///
    /// On a successful call it returns a signature signed by other party. Later this
    /// signature is combined with our signature signed, and sent to the contract
    ///
    /// * `address` - An ETH address of the other party
    fn propose(&self, address: Address) -> Box<Future<Item = Signature, Error = Error>> {
        let factory = self.transport_factory.clone();

        Box::new(
            self.storage
                .get_channel(ChannelState::New(address))
                .and_then(move |channel| {
                    future::result(factory.create_transport_protocol(channel.url.clone()))
                        .and_then(move |transport| transport.send_proposal_request(&channel))
                }),
        )
    }

    fn new_channel(
        &self,
        address: Address,
        signature: Signature,
    ) -> Box<Future<Item = (), Error = Error>> {
        // Acquire instance of contract
        let contract = self.contract.clone();
        let storage = self.storage.clone();
        let transport = self.transport_factory.clone();

        Box::new(
            self.storage
                // Channel is in New state and we know only address of the counterparty
                .get_channel(ChannelState::New(address))
                .and_then(move |mut channel| {
                    contract
                        .new_channel(
                            channel.address_a.clone(),
                            channel.address_b.clone(),
                            channel.balance_a.clone(),
                            channel.balance_b.clone(),
                            Signature::new(1u64.into(), 2u64.into(), 3u64.into()), // TODO: Prepare our own signature (placeholder)
                            signature, // Signature we got from proposal step
                            100u64.into(),
                            200u64.into(),
                        ).and_then(move |channel_id| {
                            // We know the channel is opened on chain and we should
                            // update the local state of the channel since we can now
                            // identify this channel using channel ID.
                            let old_state = channel.state.clone();
                            channel.state = ChannelState::Open(channel_id);
                            Ok((old_state, channel))
                        })
                }).and_then(move |(old_state, channel)| {
                    // Update a channel in the storage after changing its state
                    storage
                        .update_channel(old_state, channel.clone())
                        .and_then(move |_| {
                            // Pass the channel structure as we still need it
                            Ok(channel)
                        })
                }).and_then(move |channel| {
                    // Create a transport for a given url
                    transport
                        .create_transport_protocol(channel.url.clone())
                        .and_then(move |transport| Ok((transport, channel)))
                }).and_then(move |(transport, channel)| {
                    transport.send_channel_created_request(&channel)
                }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use channel_client::types::{Channel, ChannelState, UpdateTx};
    use clarity::{Address, Signature};
    #[cfg(test)]
    use double::Mock;
    #[cfg(test)]
    use payment_contract::ChannelId;
    use std::cell::RefCell;
    #[cfg(test)]
    use std::rc::Rc;
    #[cfg(test)]
    use tokio::prelude::*;
    use transport_protocol::{TransportFactory, TransportProtocol};
    /// A cloneable error to use instead of failure::Error which isn't clonable,
    /// and double expects all values to be cloneable, and hashable.
    #[derive(Fail, Debug, Clone)]
    #[cfg(test)]
    enum CloneableError {
        #[fail(display = "This is default error for {}", _0)]
        DefaultError(String),
    }

    /// This contract implementation delegates calls to trait methods into a Mock objects.
    ///
    /// Mostly to overcome the fact that double expects results to be cloneable, but
    /// Futures are not.
    struct MockContract {
        // Rc here is used for the purpose of getting another reference to the same mock object.
        mock_deposit: Rc<Mock<(Uint256), Result<(), CloneableError>>>,
        mock_withdraw: Rc<Mock<(Uint256,), Result<(), CloneableError>>>,
        mock_open_channel: Mock<(Address, Uint256, Uint256), (ChannelId)>,
        mock_join_channel: Mock<(ChannelId, Uint256), ()>,
        mock_update_channel: Mock<(ChannelId, Uint256, Uint256, Uint256, Signature, Signature), ()>,
        mock_start_challenge: Mock<(ChannelId), ()>,
        mock_close_channel: Mock<(ChannelId), ()>,
        mock_new_channel: Rc<
            Mock<
                (
                    Address,
                    Address,
                    Uint256,
                    Uint256,
                    Signature,
                    Signature,
                    Uint256,
                    Uint256,
                ),
                Result<Uint256, CloneableError>,
            >,
        >,
    }
    impl Default for MockContract {
        fn default() -> Self {
            MockContract {
                // Return a "default error" to signalize that a behaviour should be
                // modified in a test case.
                mock_deposit: Rc::new(Mock::new(Err(CloneableError::DefaultError(
                    "deposit".to_string(),
                )))),
                mock_withdraw: Rc::new(Mock::new(Err(CloneableError::DefaultError(
                    "withdraw".to_string(),
                )))),
                mock_open_channel: Mock::default(),
                mock_join_channel: Mock::default(),
                mock_update_channel: Mock::default(),
                mock_start_challenge: Mock::default(),
                mock_close_channel: Mock::default(),
                mock_new_channel: Rc::new(Mock::new(Err(CloneableError::DefaultError(
                    "new_channel".to_string(),
                )))),
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
        fn withdraw(&self, value: Uint256) -> Box<Future<Item = (), Error = Error>> {
            Box::new(
                future::result(self.mock_withdraw.call((value,)))
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

        fn new_channel(
            &self,
            address0: Address,
            address1: Address,
            balance0: Uint256,
            balance1: Uint256,
            signature0: Signature,
            signature1: Signature,
            expiration: Uint256,
            settling_period: Uint256,
        ) -> Box<Future<Item = Uint256, Error = Error>> {
            Box::new(
                future::result(self.mock_new_channel.call((
                    address0,
                    address1,
                    balance0,
                    balance1,
                    signature0,
                    signature1,
                    expiration,
                    settling_period,
                ))).from_err()
                .into_future(),
            )
        }
    }

    #[derive(Clone)]
    struct MockTransport {
        mock_send_proposal_request: Rc<Mock<(Channel), Result<Signature, CloneableError>>>,
        mock_send_channel_created_request: Rc<Mock<(Channel,), Result<(), CloneableError>>>,
    }
    impl Default for MockTransport {
        fn default() -> Self {
            Self {
                mock_send_proposal_request: Rc::new(Mock::new(Err(CloneableError::DefaultError(
                    "send_proposal_request".to_string(),
                )))),
                mock_send_channel_created_request: Rc::new(Mock::new(Err(
                    CloneableError::DefaultError("send_channel_created_request".to_string()),
                ))),
            }
        }
    }

    impl TransportProtocol for MockTransport {
        /// Send a proposal to other party
        fn send_proposal_request(
            &self,
            channel: &Channel,
        ) -> Box<Future<Item = Signature, Error = Error>> {
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
            Box::new(
                future::result(
                    self.mock_send_channel_created_request
                        .call((channel.clone(),)),
                ).from_err()
                .into_future(),
            )
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
                    CloneableError::DefaultError("create_transport_protocol".to_string()),
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

    struct MockStorage {
        mock_register_channel:
            Rc<Mock<(String, Address, Address, Uint256, Uint256), Result<Channel, CloneableError>>>,
        mock_get_channel: Rc<Mock<(ChannelState), Result<Channel, CloneableError>>>,
        mock_update_channel: Rc<Mock<(ChannelState, Channel), Result<(), CloneableError>>>,
    }

    impl Default for MockStorage {
        fn default() -> Self {
            Self {
                mock_register_channel: Rc::new(Mock::new(Err(CloneableError::DefaultError(
                    "register_channel".to_string(),
                )))),
                mock_get_channel: Rc::new(Mock::new(Err(CloneableError::DefaultError(
                    "get_channel".to_string(),
                )))),
                mock_update_channel: Rc::new(Mock::new(Err(CloneableError::DefaultError(
                    "update_channel".to_string(),
                )))),
            }
        }
    }

    impl ChannelStorage for MockStorage {
        fn register_channel(
            &self,
            url: String,
            address0: Address,
            address1: Address,
            balance0: Uint256,
            balance1: Uint256,
        ) -> Box<Future<Item = Channel, Error = Error>> {
            Box::new(
                future::result(
                    self.mock_register_channel
                        .call((url, address0, address1, balance0, balance1)),
                ).from_err()
                .into_future(),
            )
        }
        fn get_channel(&self, state: ChannelState) -> Box<Future<Item = Channel, Error = Error>> {
            Box::new(
                future::result(self.mock_get_channel.call((state)))
                    .from_err()
                    .into_future(),
            )
        }
        fn update_channel(
            &self,
            state: ChannelState,
            channel: Channel,
        ) -> Box<Future<Item = (), Error = Error>> {
            Box::new(
                future::result(self.mock_update_channel.call((state, channel)))
                    .from_err()
                    .into_future(),
            )
        }
    }

    #[test]
    fn deposit() {
        let storage = MockStorage::default();
        let contract = MockContract::default();
        // let transport = RefCell::new(MockTransport::default());
        let factory = MockTransportFactory::default();

        // Specify behaviour for deposit() contract call
        let mock_deposit = contract.mock_deposit.clone();
        mock_deposit.return_ok(());

        // Specify behaviour for transport
        let mock_create_transport_protocol = factory.mock_create_transport_protocol.clone();

        let guac = Guac::new(Box::new(contract), Box::new(factory), Box::new(storage));
        guac.deposit(123u64.into()).wait().unwrap();

        // Verify calls to the contract happened
        assert!(mock_deposit.has_calls_exactly(vec![Uint256::from(123u64)]));
    }

    #[test]
    fn withdraw() {
        let storage = MockStorage::default();
        let contract = MockContract::default();
        let factory = MockTransportFactory::default();

        // Specify behaviour for deposit() contract call
        let mock_withdraw = contract.mock_withdraw.clone();
        mock_withdraw.return_ok(());

        let guac = Guac::new(Box::new(contract), Box::new(factory), Box::new(storage));
        guac.withdraw(456u64.into()).wait().unwrap();

        // Verify calls to the contract happened
        assert!(mock_withdraw.has_calls_exactly(vec![(Uint256::from(456u64),)]));
    }

    #[test]
    fn register() {
        let storage = MockStorage::default();
        let contract = MockContract::default();
        let transport = RefCell::new(Box::new(MockTransport::default()));
        let factory = MockTransportFactory::default();

        // Specify behaviour for deposit() contract call
        let mock_register = storage.mock_register_channel.clone();

        // TODO: Addresses should be reordered before going into storage

        // "us"
        let address0: Address = "0x0000000000000000000000000000000000000001"
            .parse()
            .unwrap();
        let balance0: Uint256 = 1000u64.into();
        // "them"
        let address1: Address = "0x0000000000000000000000000000000000000002"
            .parse()
            .unwrap();
        let balance1: Uint256 = 0u64.into();

        assert!(address0 < address1);

        let channel = Channel {
            state: ChannelState::New(address1.clone()),
            address_a: address0.clone(),
            address_b: address1.clone(),
            deposit_a: balance0.clone(),
            deposit_b: balance1.clone(),
            challenge: 0u64.into(),
            nonce: 0u64.into(),
            close_time: 0u64.into(),
            balance_a: balance0.clone(),
            balance_b: balance1.clone(),
            is_a: true,
            url: "42.42.42.42:4242".to_string(),
        };
        mock_register.return_ok(channel.clone());

        // Specify behaviour for transport
        let mock_create_transport_protocol = factory.mock_create_transport_protocol.clone();
        mock_create_transport_protocol.use_closure(Box::new(move |_params| {
            // This will always return clones of the same transport instance.
            let instance = transport.clone();
            Ok(instance)
        }));

        let guac = Guac::new(Box::new(contract), Box::new(factory), Box::new(storage));

        let address0: Address = "0x0000000000000000000000000000000000000001"
            .parse()
            .unwrap();
        let address1: Address = "0x0000000000000000000000000000000000000002"
            .parse()
            .unwrap();

        guac.register_counterparty(
            "42.42.42.42:4242",
            address0.clone(),
            address1.clone(),
            42u64.into(),
            0u64.into(),
        ).wait()
        .unwrap();

        // Verify that counterparty is registered
        assert!(mock_register.has_calls_exactly(vec![(
            "42.42.42.42:4242".to_string(),
            address0,
            address1,
            42u64.into(),
            0u64.into()
        )]));
    }

    #[test]
    fn propose() {
        let storage = MockStorage::default();
        let contract = MockContract::default();
        let transport = RefCell::new(Box::new(MockTransport::default()));
        let factory = MockTransportFactory::default();

        // Addresses should be reordered before going into storage

        // "us"
        let address0: Address = "0x0000000000000000000000000000000000000001"
            .parse()
            .unwrap();
        // "them"
        let address1: Address = "0x0000000000000000000000000000000000000002"
            .parse()
            .unwrap();

        assert!(address0 < address1);

        let mock_channel = Channel {
            state: ChannelState::New(address1.clone()), // Channel ID is not yet know
            address_a: address0.clone(),
            address_b: address1.clone(),
            deposit_a: 0u64.into(),
            deposit_b: 0u64.into(),
            challenge: 0u64.into(),
            nonce: 0u64.into(),
            close_time: 0u64.into(),
            balance_a: 0u64.into(),
            balance_b: 0u64.into(),
            is_a: true,
            url: "42.42.42.42:4242".to_string(),
        };
        // Specify behaviour for deposit() contract call
        let mock_get_channel = storage.mock_get_channel.clone();

        // Channel is already registered in storage
        mock_get_channel.return_ok(mock_channel.clone());

        // Channel is already registered in storage
        let mock_propose = transport.borrow().mock_send_proposal_request.clone();

        let correct_signature = Signature::new(10u64.into(), 20u64.into(), 30u64.into());

        // Other node returns a valid signature
        mock_propose.return_ok(correct_signature.clone());

        // Specify behaviour for transport
        let mock_create_transport_protocol = factory.mock_create_transport_protocol.clone();
        mock_create_transport_protocol.use_closure(Box::new(move |_params| {
            // This will always return clones of the same transport instance.
            let instance = transport.clone();
            Ok(instance)
        }));

        let guac = Guac::new(Box::new(contract), Box::new(factory), Box::new(storage));
        let res = guac.propose(address1.clone()).wait().unwrap();
        assert_eq!(res, correct_signature);

        // Verify calls to the contract happened
        assert!(
            mock_create_transport_protocol.has_calls_exactly(vec!["42.42.42.42:4242".to_string()])
        );
        assert!(
            mock_propose.has_calls_exactly(vec![mock_channel.clone()]),
            "Proposal not sent to other node!"
        );
    }

    #[test]
    fn new_channel() {
        let storage = MockStorage::default();
        let contract = MockContract::default();
        let transport = RefCell::new(Box::new(MockTransport::default()));
        let factory = MockTransportFactory::default();

        // This will use address pair that will be reordered when passed to contract
        let address0: Address = "0x0000000000000000000000000000000000000001"
            .parse()
            .unwrap();
        let address1: Address = "0x0000000000000000000000000000000000000002"
            .parse()
            .unwrap();
        assert!(address0 < address1);

        let mock_channel = Channel {
            // This is the node we registered by its Address
            state: ChannelState::New(address1.clone()),
            address_a: address0.clone(),
            address_b: address1.clone(),
            deposit_a: 1000u64.into(),
            deposit_b: 0u64.into(),
            challenge: 0u64.into(),
            nonce: 0u64.into(),
            close_time: 0u64.into(),
            balance_a: 1000u64.into(),
            balance_b: 0u64.into(),
            is_a: true,
            url: "42.42.42.42:4242".to_string(),
        };

        // Specify behaviour for deposit() contract call
        let mock_get_channel = storage.mock_get_channel.clone();

        // Channel is already registered in storage
        mock_get_channel.return_ok(mock_channel.clone());

        let mock_update_channel = storage.mock_update_channel.clone();
        mock_update_channel.return_ok(());

        let mock_new_channel = contract.mock_new_channel.clone();
        mock_new_channel.return_ok(Uint256::from(42u64));

        // Send notification will return success
        let mock_send_channel_created_request =
            transport.borrow().mock_send_channel_created_request.clone();
        mock_send_channel_created_request.return_ok(());

        // Specify behaviour for transport
        let mock_create_transport_protocol = factory.mock_create_transport_protocol.clone();
        mock_create_transport_protocol.use_closure(Box::new(move |_params| {
            // This will always return clones of the same transport instance.
            let instance = transport.clone();
            Ok(instance)
        }));

        let guac = Guac::new(Box::new(contract), Box::new(factory), Box::new(storage));
        let res = guac
            .new_channel(
                address1.clone(),
                Signature::new(4u64.into(), 5u64.into(), 6u64.into()),
            ).wait()
            .unwrap();

        // Verify calls to the contract happened
        assert!(
            mock_create_transport_protocol.has_calls_exactly(vec!["42.42.42.42:4242".to_string()])
        );
        assert!(
            mock_new_channel.has_calls_exactly(vec![(
                address0.clone(),
                address1.clone(),
                1000u64.into(),
                0u64.into(),
                Signature::new(1u64.into(), 2u64.into(), 3u64.into()), // TODO: This is our signature and this is a placeholder
                Signature::new(4u64.into(), 5u64.into(), 6u64.into()),
                100u64.into(), // exp
                200u64.into(), // settling
            )]),
            "Proposal not sent to other node!"
        );

        assert!(
            mock_update_channel.has_calls_exactly(vec![(
                ChannelState::New(address1.clone()),
                Channel {
                    // This is the channel we recevied from newChannel contract call
                    state: ChannelState::Open(42u64.into()),
                    address_a: address0.clone(),
                    address_b: address1.clone(),
                    deposit_a: 1000u64.into(),
                    deposit_b: 0u64.into(),
                    challenge: 0u64.into(),
                    nonce: 0u64.into(),
                    close_time: 0u64.into(),
                    balance_a: 1000u64.into(),
                    balance_b: 0u64.into(),
                    is_a: true,
                    url: "42.42.42.42:4242".to_string(),
                },
            )]),
            "Update channel is not called with proper arguments"
        );

        assert!(
            mock_send_channel_created_request.has_calls_exactly(vec![(Channel {
                // This is the channel we recevied from newChannel contract call
                state: ChannelState::Open(42u64.into()),
                address_a: address0.clone(),
                address_b: address1.clone(),
                deposit_a: 1000u64.into(),
                deposit_b: 0u64.into(),
                challenge: 0u64.into(),
                nonce: 0u64.into(),
                close_time: 0u64.into(),
                balance_a: 1000u64.into(),
                balance_b: 0u64.into(),
                is_a: true,
                url: "42.42.42.42:4242".to_string(),
            },)]),
            "Send channel created notification not sent"
        );
    }
}
