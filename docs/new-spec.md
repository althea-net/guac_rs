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

### Proposing a channel

`POST /propose`

Proposes a channel from party A to party B.

Request parameters:

- `channel_id` - A string that represents 32 bytes in a hexadecimal form with `0x` prefix as explained in [Serializing binary data](#serializing-binary-data) . This string matches the regexp `0x[a-f0-9]{64}`.
- `address0` - Address of proposing party's address
- `address1` - Address of the other party's address
- `balance0` - Deposit of proposing party
- `balance1` - Initial deposit of the other party

Possible responses:

- `HTTP 201 CREATED`

Party B accepts the proposal automatically and stores the information about the channel in memory with `Proposed` state. This means that party B expects a confirmation with `POST /channel_created` that the channel is created on the network.

Parameters:

- `signature` - Signed a fingerprint which is defined as following

  ```rust
  let fingerprint = keccak256(abi.encodePacked("newChannel", address0, address1, balance0, balance1)
  ```

- `HTTP 400 BAD REQUEST`

Party B considers the request invalid (i.e. malicious balances, address0 is on blacklist, address1 is not owned by party B, etc.). Although possible response, this request is meant to succeed with `HTTP 201 CREATED` response in most of the cases.

### Confirming a channel

- `POST /channel_created`

A request sent from proposing party (party A) to party B to notify it about the fact that the channel is open on the network.

Request parameters:

- `channel_id` - An existing channel ID that was used to open a channel on the network, and is expected to be proposed already on the calling guac node.

Possible responses:

- `HTTP 201 NO CONTENT`

Notification succeed. B knows that the channel is opened already, and the state will be updated after that to `Created`.

### Closing a channel

- `POST /close_channel_fast`

A request sent from party A to party B to notify it about the intention to close the channel.

Request parameters:

- `channel_id` - Channel ID
- `nonce` - A non-decreasing seqeuence number for given Channel ID
- `balance0` - Current balance
- `balance1` - Current balance of the other party

Possible responses:

- `HTTP 200 OK`

Request succeed. Respond with a signed fingerprint for a given operation that is computed as:

```rust
fingerprint = keccak256(abi.encodePacked("closeChannelFast", channel_id, nonce, balance0, balance1))
```

Parameters of the response:

```
fingerprint=0x...
```

- `HTTP 400 BAD REQUEST`

The request was invalid for any reason (invalid or wrong parameters, invalid address, malformed parameters).

### Refilling a channel

- `POST /redraw`

This request signalizes the intention to refill the channel. To do that it first needs to contact other party about this and receive a valid signature to allow a contract call for redraw.

Request parameters:

- `channel_id` - Channel ID
- `nonce` - A non-decreasing seqeuence number for given Channel ID
- `balance0` - Deposit of proposing party
- `balance1` - Initial deposit of the other party

Responses:

- `HTTP 200 OK`

Redraw request succeed. Response parameters:

- `signature` fingerprint is defined as

  ```rust
  let fingerprint = keccak256(abi.encodePacked("closeChannelFast", channel_id, nonce, balance0, balance1))
  ```

- `HTTP 400 BAD REQUEST`

The request was invalid for any reason (invalid or wrong parameters, invalid address, malformed parameters).

## Contract layer

This component should reflect the functionality of the guac payment channel contract

# Implementation

## CounterpartyApi

A trait that describes the node to node protocol described above in section [Transport layer](#transport-layer).

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

trait CounterpartyApi {
    /// Proposes a channel and returns Signature after signing a fingerprint
    fn propose(&mut self, channel: Channel) -> impl Future<Item = Signature, Error = Error>;
    /// Notifies about channel created
    fn channel_created(&mut self, channel: Channel) -> impl Future<Item = (), Error = Error>;
    /// Update state
    fn update(&mut self, channel: Channel) -> impl Future<Item = (), Error = Error>;
    /// Redraw request to other party to refill or withdraw
    /// Proposes a channel and returns Signature after signing a fingerprint
    fn redraw(&mut self, channel: Channel) -> impl Future<Item = Signature, Error = Error>;
};

struct HTTPTransportClient {
    /// TODO: Implementation details about the client such as base URL, etc.
};

impl CounterpartyApi for HTTPTransportClient {
    /// Send `POST http://url/propose` request to other party
    fn propose(&mut self, channel: Channel) -> impl Future<Item = Signature, Error = Error>;
    /// Notifies about channel created with `POST http://url/channel_created`
    fn channel_created(&mut self, channel: Channel) -> impl Future<Item = (), Error = Error>;
    /// Updates state with `POST http://url/update`
    fn update(&mut self, channel: Channel) -> impl Future<Item = (), Error = Error>;
    /// Requests refill or withdraw with `POST http://url/redraw`
    fn redraw(&mut self, channel: Channel) -> impl Future<Item = Signature, Error = Error>;
}

struct HTTPTransportServer {
    /// TODO: Implementation details about the server such HTTP as channel storage etc.
    /// TODO: Instance of HTTP server that would define actual HTTP endpoints should call appropriate methods on instance of this
}

impl CounterpartyApi for HTTPTransportServer {
    /// When receiving `POST http://url/propose` request from other party
    fn propose(&mut self, channel: Channel) -> impl Future<Item = Signature, Error = Error>;
    /// When received `POST http://url/channel_created` about channel is created
    fn channel_created(&mut self, channel: Channel) -> impl Future<Item = (), Error = Error>;
    /// Update state of channel
    fn update(&mut self, channel: Channel) -> impl Future<Item = (), Error = Error>;
    /// Handle refill or redraw request
    fn redraw(&mut self, channel: Channel) -> impl Future<Item = Signature, Error = Error>;
}
```

### PaymentContract

This is the implementation of payment channel trait.

In contrast to PaymentProtocol parameters of methods in this trait resembles the arguments of the contract itself as closely as possible. This way in case of ABI changes every use of each method will be thorughly reviewed before updating the trait and its implementations.

```rust
trait PaymentContract {
    /// Creates new channel with given parameters
    fn new_channel(&self,
        channel_id: &ChannelId,
        address0: &Address,
        address1: &Address,
        balance0: &Uint256,
        balance1: &Uint256,
        signature_0: &Signature,
        signature_1: &Signature) -> impl Future<Item = (), Error = Error>;
    /// Updates the state of the contract with given parameters.
    ///
    /// Precondition: Channel identified by channel_id has to be created with new_channel before
    /// Postcondition: Channel parameters will be updated with new parameters
    fn update_state(&self,
        channel_id: &ChannelId,
        sequence_number: &Uint256,
        balance0: &Uint256,
        balance1: &Uint256,
        signature_0: &Signature,
        signature_1: &Signature) -> impl Future<Item = (), Error = Error>;
    /// Closes channel fast
    fn close_channel_fast(&self,
        channel_id: &ChannelId,
        sequence_number: &Uint256,
        balance0: &Uint256,
        balance1: &Uint256,
        signature_0: &Signature,
        signature_1: &Signature) -> impl Future<Item = (), Error = Error>;
    /// REfill or withDRAW
    fn redraw(&self,
        channel_id: &ChannelId,
        sequence_number: &Uint256,
        balance0: &Uint256,
        balance1: &Uint256,
        signature_0: &Signature,
        signature_1: &Signature) -> impl Future<Item = (), Error = Error>;
}
```

### PaymentManager

This is what `guac_actix`'s `PaymentController` used to be, but is not tied to actix now. This more likely extracts its functionality in a generic layer. Exposing it through Actix actor will be trivial though, as explained later in [Actix adapter](#actix-adapter) section.

```rust

/// Tries to resemble most of the guac_actix's PaymentController stuff
trait PaymentManager {
    /// This message needs to be sent periodically for every single address the application is
    /// interested in, and it returns the amount of money we can consider to have "received"
    /// from a counterparty
    fn withdraw(&self) -> impl Future<Item = Uint256, Error = Error>;
    /// Makes payment to other party.
    fn make_payment(&self, counterparty: &Counterparty, value: &Uint256) -> impl Future<Item = (), Error = Error>;
    /// Open channel
    fn open_channel(&self, counterparty: &Counterparty) -> impl Future<Item = (), Error = Error>;
}
```

## Actix adapter

`uac_actix` crate is just meant to expose functionality of `guac_core`. Other than that, it also defines a HTTP server for interacting with `HTTPTransportServer`. This gives the benefit of keeping both client and server in sync without splitting it off to different crates.

Compared to previous design it will not use `Tick` messages to progress the state machine. Instead, whole functionality and business logic will be implemented inside `guac_core` using futures.

# Appendix

## Serializing binary data

A binary data of any size have to be expressed as its hexadecimal representation with `0x` prefix. Length of the string is always even, and greater than 2.

Examples of valid binary data serialized to strings:

- `0x6333c17b8b1e713b002079309152e2c4ff26f2ab70d428486c5addd9da4695e2`

This convention would make it clear on the other side that the string contains binary data that can be deserialized easily.

## Channel IDs

Generating Channel IDs should be done in a securely manner using a random device (think of `/dev/urandom`). Channel ID has to be exacly 32 bytes in length and that exactly matches `bytes32` type in Solidity.
