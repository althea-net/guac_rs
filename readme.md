# Guac light client

Guac is an Ethereum single hop payment channel client for the [Guac](https://github.com/althea-mesh/guac)
payment channel contract. This light client is able to send and verify channel opening, updating, and
ending transactions. It relies on one or several bounty hunting full nodes which relay transactions
onto the blockchain. These bounty hunting nodes are not able to spend any money without the
permission of this light client, but they could censor its transactions, in the worst case leading
to the loss of some funds stored in the channel. For this reason it is advisable to connect to
several bounty hunting nodes.

# TODO:

- Finish unit tests
- Add checkAccrual (formerly withdraw) function to userAPI
- Add bounty hunter updates
- Add real signature verification
- Add withdraw function to userApi

# Guac APIs

Guac effectively has 2 APIs- a counterparty API which is called by other Guac nodes, and a user api which is called by the user (or a piece of software acting on behalf of the user).

## Counterparty API

This is called by other Guac nodes.

### Propose Channel

Asks a counterparty to sign a newChannel contract tx

Endpoint: /propose_channel

Request data type: `Channel` (basically all the info you need to call the
`newChannel` contract call.

return type: Signature on the newChannel contract tx

### Propose ReDraw

Asks a counterparty to sign a reDraw contract tx, to add or withdraw money on our side from the channel.

Endpoint: /propose_redraw

Request data type: TBD

return type: Signature on the newChannel call

### Update

Tells your counterparty about your new state

Endpoint: /update

Request data type: `UpdateTx`
Return data type: `UpdateTx` (containing the newest transaction data from their local state)

### ChannelOpened notification

Notifies a counterparty who has just responded affirmatively to a propose channel call that the channel has been opened on the blockchain.

Request data type: `Channel`

return data type: `null`

### ReDraw notification

Notifies a counterparty who has just responded affirmatively to a reDraw call that the channel has been reDrawn on the blockchain.

Request data type: `Channel`

return data type: `null`

## User API

This is called by the user (or a piece of software acting on behalf of the user).

### Register

This is used to register a new counterparty.

### Fill Channel

This is used to open a channel with a counterparty that we wish to pay in the future. This incurs a gas cost.

### Make Payment

This is used to make a payment to a counterparty. This does not incur a gas cost.

### Check Accrual

NOTE: This is currently called "Withdraw" in the code. It needs to be renamed to avoid confusion.

This method is somewhat nuanced. It is used to check how much payment has been received from a counterparty since the last time the method was called. It does not take payments we send the counterparty into account. That is, if you send me 10, then I send you 5, then I call "Check Accrual", I will see that I have received 10 from you.

for example:

- A: 100, B: 100
- A <-10-- B
- A: 110, B: 90
- A calls "Check Accrual": 10
- A calls "Check Accrual": 0

---

- A <-10-- B
- A: 120, B:80
- A --5-> B
- A: 115, B:85
- A calls "Check Accrual": 10

---

- A --10-> B
- A: 105, B: 95
- A <-5-- B
- A calls "Check Accrual": 5

### Withdraw

This allows you to withdraw some or all of your balance from a channel. This incurs a gas cost.

### Refill

This allows you to refill a channel that is getting low to avoid a disruption of service by not being able to pay a counterparty while a new channel is being opened. This incurs a gas cost.

### Close

This is used to close a channel. Under the hood, it calls the blockchain to start the channel's challenge period. Then it tells the counterparty to call the close channel function on the contract, resulting in a fast close.

# File structure

Guac is structured into 3 modules, guac_core which consists of the implementation of the channel
update logic as well as interaction with the blockchain through bounty hunters. guac_actix is an
interface over guac_core which sends and receives messages from the external world via HTTP, but
receives commands via actix. guac_http makes the command interface HTTP instead to provide wider
interoperability.

# Development

Set up git hooks:

```sh
cd .git/hooks/
ln -s ../../.git-hooks/pre-commit
```

## Running tests

Tests are run from the `guac_http` crate, which imports logic from `guac_core`. To prepare your environment for testing, you can run the `./scripts/local-setup.sh` script. This will clone the `althea-mesh/guac` Github repo into `./.contract`, start an instance of Ganache in the background (Ganache is a local Ethereum chain), compile and deploy the contract to this chain, and save the address of the contract and two test accounts into `./guac_http/config.toml`, where they can be read by the tests.

If you want to test against a new version of `althea-mesh/guac`, delete the `./.contract` folder and run the `./scripts/local-setup.sh` script again.

`./scripts/local-setup.sh` logs verbose output from Ganache to ./.ganache-log.

WARNING: `./scripts/local-setup.sh` will stop any process running on port 8545.

`guac_http` uses the `config_struct` crate to load
