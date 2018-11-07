
# Introduction

Guac is an Ethereum single hop payment channel client for guac Bi-Directional Payment
Channel contract. This light client is able to send and verify channel opening, updating, and ending
transactions.

# Problem

Guac_rs used to implement Ethcalate (aka Connectix) contract client https://github.com/althea-mesh/simple-bidirectional-erc20-channel/ to implement the payment logic. That contract code turns out to have more issues than expected and we decided to move back to the original guac contract located at https://github.com/althea-mesh/guac/.

With the new implementation of the payment logic we want to focus on the reusability and make guac_rs generic enough to be able to be reused in other projects as well, while at the same it should ship ready to use implementation that other projects can use straight away.

# Components

There are various components that are used to communicate between parties off-chain (i.e. through HTTP) and on-chain (i.e. Web3 protocol).

## Transport layer

A set of functions that allows a set of instances of Guac_rs to communicate with each other over a reliable transport layer such as HTTP.

_Open question: What about making it a proper RESTful API i.e. POST /channel, DELETE /channel/id for closing, POST /channel/:id/created?_

### Proposing a channel

`POST /propose`

Proposes a channel from party A to party B.

Request parameters:

- `channel_id` - A random 32 bytes string that matches the regexp `[a-f0-9]{32}`.
- `address0` - Address of proposing party's address
- `address1` - Address of the other party's address
- `balance0` - Deposit of proposing party
- `balance1` - Initial deposit of the other party

Possible responses:

- `HTTP 201 CREATED`

Party B accepts the proposal automatically and stores the information about the channel in memory with `Proposed` state. This means that party B expects a confirmation with `POST /channel_created` that the channel is created on the network.

Parameters:

- `signature` - Signed a fingerprint _TODO: Describe how to derive fingerprint for proposal stage_

- `HTTP 400 BAD REQUEST`

Party B considers the request invalid (i.e. malicious balances, address0 is on blacklist, address1 is not owned by party B, etc.). Although possible response, this request is meant to succeed with `HTTP 201 CREATED` response in most of the cases.

### Confirming a channel

- `POST /channel_created`

A request sent from proposing party (party A) to party B to notify it about the fact that the channel is open on the network. Party B is required to check if the channel is in fact open by querying a contract _Open question: How?_

Request parameters:

- `channel_id` - An existing channel ID that was used to open a channel on the network, and is expected to be proposed already on the calling guac node.

Possible responses:

- `HTTP 201 NO CONTENT`

Notification succeed. B knows that the channel is opened already, and the state will be updated after that to `Created`.

## Contract layer

This component should reflect the functionality of the guac payment channel contract

# Implementation

## CryptoService

```rust
trait CryptoService {
    fn init(&self, config: &Config) -> Result<(), Error>;
    fn own_eth_addr(&self) -> Address;
    fn secret(&self) -> PrivateKey;
    fn secret_mut<'ret, 'me: 'ret>(&'me self) -> RwLockWriteGuardRefMut<'ret, Crypto, PrivateKey>;
    fn get_balance_mut<'ret, 'me: 'ret>(&'me self)
        -> RwLockWriteGuardRefMut<'ret, Crypto, Uint256>;
    /// Access local balance without querying network.
    ///
    /// This is different from get_network_balance, where an actual
    /// network call is made to retrieve up to date balance. This method
    /// should be preferred over querying the network.
    fn get_balance(&self) -> Uint256;
    fn eth_sign(&self, data: &[u8]) -> Signature;
    fn hash_bytes(&self, x: &[&[u8]]) -> Uint256;
    fn verify(_fingerprint: &Uint256, _signature: &Signature, _address: Address) -> bool;
    fn web3<'ret, 'me: 'ret>(&'me self) -> RwLockReadGuardRef<'ret, Crypto, Web3Handle>;

    // Async stuff
    fn get_network_id(&self) -> impl Future<Item = u64, Error = Error>;
    fn get_nonce(&self) -> impl Future<Item = Uint256, Error = Error>;
    fn get_gas_price(&self) -> impl Future<Item = Uint256, Error = Error>;
    /// Queries the network for current balance. This is different
    /// from get_balance which keeps track of local balance to save
    /// up on network calls.
    ///
    /// This function shouldn't be called every time. Ideally it should be
    /// called once when initializing private key, or periodically to synchronise
    /// local and network balance.
    fn get_network_balance(&self) -> impl Future<Item = Uint256, Error = Error>;
    /// Waits for an event on the network using the event name.
    ///
    /// * `event` - Event signature
    /// * `topic` - First topic to filter out
    fn wait_for_event(
        &self,
        event: &str,
        topic: Option<[u8; 32]>,
    ) -> impl Future<Item = Log, Error = Error>;
    /// Broadcast a transaction on the network.
    ///
    /// * `action` - Defines a type of transaction
    /// * `value` - How much wei to send
    fn broadcast_transaction(
        &self,
        action: Action,
        value: Uint256,
    ) -> impl Future<Item = Uint256, Error = Error>;
}

## Storage

This is a trait that implements a storage of Counterparties. It is left in tact for most of the part, only the interface is extracted as a trait.

```rust
// Open question: Naming of this trait?
trait Storage {
    pub fn get_all_counterparties(&self) -> impl Future<Item = Vec<Counterparty>, Error = Error>;
    pub fn get_all_channel_managers_mut(
        &self,
    ) -> impl Future<Item = Vec<Guard<ChannelManager>>, Error = Error>;
    pub fn get_channel(
        &self,
        k: Address,
    ) -> impl Future<Item = Guard<ChannelManager>, Error = Error>;
    fn init_data(
        &self,
        k: Counterparty,
        v: ChannelManager,
    ) -> impl Future<Item = (), Error = Error>;
}
```

## TransportProtocol

A trait that describes the node to node protocol described above in section [Transport layer](#transport-layer).

_Open question: How do we name those traits and implementations? For now its example to illustrate the idea_

```rust
type ChannelId = [u8; 32];

struct Channel {
    channel_id: ChannelId,
    address0: Address,
    address1: Address,
    balance0: Uint256,
    balance1: Uint256,
    // Rest of implementation details about the channel
}

trait TransportProtocol {
    /// Proposes a channel and returns Signature after signing a fingerprint
    fn propose(&mut self, channel: &Channel) -> impl Future<Item = Signature, Error = Error>;
    /// Notifies about channel created
    fn channel_created(&mut self, channel_id: &ChannelId) -> impl Future<Item = (), Error = Error>;
    /// Update state
    fn update(&mut self, channel_id: &ChannelId, ...) -> impl Future<Item = (), Error = Error>;
};
 
struct HTTPTransportClient {
    /// TODO: Implementation details about the client such as base URL, etc. 
};

impl TransportProtocol for HTTPTransportClient {
    /// Send `POST http://url/propose` request to other party
    fn propose(&mut self, channel: &Channel) -> impl Future<Item = Signature, Error = Error>;
    /// Notifies about channel created with `POST http://url/channel_created`
    fn channel_created(&mut self, channel: &Channel) -> impl Future<Item = (), Error = Error>;
    /// TODO: Update state with `POST http://url/update`
    fn update(&mut self, channel: &Channel) -> impl Future<Item = (), Error = Error>;
}

struct HTTPTransportServer {
    /// TODO: Implementation details about the server such HTTP as channel storage etc.
    /// TODO: Instance of HTTP server that would define actual HTTP endpoints should call appropriate methods on instance of this 
}

impl TransportProtocol for HTTPTransportServer {
    /// When receiving `POST http://url/propose` request from other party
    fn propose(&mut self, channel: &Channel) -> impl Future<Item = Signature, Error = Error>;
    /// When received `POST http://url/channel_created` about channel is created
    fn channel_created(&mut self, channel: &Channel) -> impl Future<Item = (), Error = Error>;
    /// TODO: Update state with `POST http://url/update`
    fn update(&mut self, channel: &Channel) -> impl Future<Item = (), Error = Error>;
}
```

### PaymentContract

This is the implementation of payment channel trait.

In contrast to PaymentProtocol parameters of methods in this trait resembles the arguments of the contract itself as closely as possible.

```rust
trait PaymentContract {
    /// Creates new channel with given parameters
    fn new_channel(&self, 
        channel_id: &ChannelId,
        address0: &Address,
        address1: &Address,
        balance0: &Uint256,
        balance1: &Uint256,
        signature0: &Signature,
        signature1: &Signature) -> impl Future<Item = (), Error = Error>;
    /// Updates the state of the contract with given parameters.
    /// 
    /// Precondition: Channel identified by channel_id has to be created with new_channel before
    /// Postcondition: Channel parameters will be updated with new parameters
    fn update_state(&self,
        channel_id: &ChannelId,
        sequence_number: &Uint256,
        balance0: &Uint256,
        balance1: &Uint256,
        signature0: &Signature,
        signature1: &Signature) -> impl Future<Item = (), Error = Error>;
    /// Closes channel fast
    fn close_channel_fast(&self,
        channel_id: &ChannelId,
        sequence_number: &Uint256,
        balance0: &Uint256,
        balance1: &Uint256,
        signature0: &Signature,
        signature1: &Signature) -> impl Future<Item = (), Error = Error>;
}
```

### PaymentManager

This is what `guac_actix`'s `PaymentController` used to be, but is not tied to actix now. This more likely extracts its functionality in a generic layer. Exposing it through Actix actor will be trivial though, as explained later in [Actix adapter](#actix-adapter) section.

```rust

/// Tries to resemble most of the guac_actix's PaymentController stuff
trait PaymentManager {
    fn withdraw(&self, ...) -> impl Future<Item = (), Error = Error>;
    fn make_payment(&self, counterparty: &Counterparty) -> impl Future<Item = (), Error = Error>;
}

struct GuacPaymentManager<T, C, S>
where T: TransportProtocol, C: PaymentContract, S: Storage {
    transport: TransportProtocol,
    contract: PaymentContract,
    storage: Storage,
}

struct GuacPaymentManager<T, C, S>
    where T: TransportProtocol,
          C: PaymentContract,
          S: Storage {
    fn new(t: T, c: C, s: S) -> Self {
        Self {
            transport: t,
            contract: c,
            storage: s,
        }
    }
}

impl<T, C, S> PaymentManager for GuacPaymentManager<T, C, S> {
    fn withdraw(&self, ...) -> impl Future<Item = (), Error = Error> {
        /// TODO
    }
    fn make_payment(&self, counterparty: &Counterparty, amount: Uint256) -> impl Future<Item = (), Error = Error> {
        Box::new(self.storage.get_channel(counterparty.eth_address).and_then(
            move |mut channel_manager| {
                channel_manager.pay_counterparty(Uint256(amount));
            }
        ));
    }

    fn tick(&self) -> impl Future<Item = (), Error = Error> {
        Box::new(self.storage.get_all_counterparties().and_then(|keys| {
            for i in keys {
                self.storage
                    .get_channel(counterparty.address.clone())
                    .and_then(move |mut channel_manager| {

                        let action = channel_manager.tick();

                        match action {
                            // Use self.transport for HTTP requests between parties
                            // Use self.contract for Contract requests
                            // Use self.storage for accessing counterparties
                        }
                    });
            }
        });
    }
}

```

### Channel manager

Channel manager is code is mostly left in place, just removed parts of code where it does not match the functionality in Guac contract.

```rust
enum ChannelManager {
    // This code is mostly left intact
    New,
    Proposed {
        state: Channel,
        accepted: bool,
    },
    Open {
        state: CombinedState,
    },
    Closed {
        state: CombinedState,
    }
    // ...
}

impl ChannelManager {
    // Code left intact
    fn tick(&self) -> Result<ChannelManagerAction, Error>;
}
```

## Actix adapter

_Open question: Combining PaymentManager, CryptoService and exposing their functionality through messages like MakePayment (compatibliity), GetOwnBalance (compat) etc?_

TBD

# Appendix

## Channel IDs

Generating Channel IDs should be done in a securely manner using a random device (think of `/dev/urandom`). Channel ID has to be exacly 32 bytes in length and that exactly matches `bytes32` type in Solidity.