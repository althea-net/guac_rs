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

Party B considers the request invalid (i.e. malicious balances, address0 is on blacklist, address1 is not owned by party B, etc.). Although possible response, this request is meant to succeed with `HTTP 201 CRAETED` response in most of the cases.

### Confirming a channel

- `POST /channel_created`

A request sent from proposing party (party A) to party B to notify it about the fact that the channel is open on the network. Party B is required to check if the channel is in fact open by querying a contract _Open question: How?_

Request parameters:

- `channel_id` - An existing channel ID that was used to open a channel on the network, and is expected to be proposed already on the calling guac node.

Possible responses:

- `HTTP 201 NO CONTENT`

Notification succeed. Changed the state properly.

## Contract layer

This component should reflect the functionality of the guac payment channel contract

# Implementation

_Open question: Do we want to keep tick functionality since the state machine with guac contract is simplified?_

```rust
trait Tick {
    fn tick(&self);
}
```

## TransportProtocol

A trait that describes the node to node protocol described above in section [Transport layer](#transport-layer).

_Open question: How do we name those traits and implementations? For now its example to illustrate the idea_


```rust
trait TransportProtocol {
    /// Proposes a channel
    fn propose(&mut self, channel_id: &ChannelId, address0: &Address, address1: &Address, balance0: &Uint256, balance1: &Uint256) -> impl Future<Item = Signature, Error = Error>;
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
    fn propose(&mut self, channel_id: &ChannelId, address0: &Address, address1: &Address, balance0: &Uint256, balance1: &Uint256) -> impl Future<Item = Signature, Error = Error>;
    /// Notifies about channel created with `POST http://url/channel_joined`
    fn channel_created(&mut self, channel_id: &ChannelId) -> impl Future<Item = (), Error = Error>;
    /// TODO: Update state with `POST http://url/update`
    fn update(&mut self, channel_id: &ChannelId, ...) -> impl Future<Item = (), Error = Error>;
}

struct HTTPTransportServer {
    /// TODO: Implementation details about the server such HTTP as channel storage etc.
    /// TODO: Instance of HTTP server that would define actual HTTP endpoints should call appropriate methods on instance of this 
}

impl TransportProtocol for HTTPTransportServer {
    /// When receiving `POST http://url/propose` request from other party
    fn propose(&mut self, channel_id: &ChannelId, address0: &Address, address1: &Address, balance0: &Uint256, balance1: &Uint256) -> impl Future<Item = Signature, Error = Error>;
    /// When received `POST http://url/channel_joined` about channel is created
    fn channel_created(&mut self, channel_id: &ChannelId) -> impl Future<Item = (), Error = Error>;
    /// TODO: Update state with `POST http://url/update`
    fn update(&mut self, channel_id: &ChannelId, ...) -> impl Future<Item = (), Error = Error>;
}
```

# Appendix

## Channel IDs

Generating Channel IDs should be done in a securely manner using a random device (think of `/dev/urandom`) and the channel ID has to be exacly 32 bytes in length.