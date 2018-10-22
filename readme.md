# Guac light client

Guac is an Ethereum single hop payment channel client for the Ethcalate Bi-Directional Payment
Channel. This light client is able to send and verify channel opening, updating, and ending
transactions. It relies on one or several bounty hunting full nodes which relay transactions
onto the blockchain. These bounty hunting nodes are not able to spend any money without the
permission of this light client, but they could censor its transactions, in the worst case leading
to the loss of some funds stored in the channel. For this reason it is advisable to connect to
several bounty hunting nodes.

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

This is a work in progress of integration tests that will involve state changes, and actual contract calls.

This test expects $CONTRACT_ADDRESS variable, and $GANACHE_HOST for a
network.

# Running the tests

Easiest way is through containerized ganache server.

First step is to run a ganache server in a container:

```sh
docker run --rm -it -p 8545:8545 trufflesuite/ganache-cli:latest -a 10 -e 100000  --debug
```

* `-a 10` generate 10 accounts 
* `-e 100000` initial balance of 100000ETH
* `--debug` more verbose

(keep in mind that this container is ran with --rm -it so it means after CTRL+C whole container is removed with the data inside. Simply stop it and start for a clean state.)

After that you have ganache server listening on port 8545.

Next step is to deploy contract:

```sh
cd ~/simple-bidirectional-erc20-channel
truffle migrate
```

Example output:

```
truffle migrate
Using network 'development'.

Running migration: 1_initial_migration.js
  Deploying Migrations...
  ... 0x4ecd0489d3f58e0314b12defa26f0ed1d2a805f93d1adcda1cd86a01e4e7dd9b
  Migrations: 0xbb8a064e941d89388641587e73e534ee8985ca20
Saving successful migration to network...
  ... 0x51832a8ce2741bdff661af546076b5d9580b15d28371648f415c63d05f1205ae
Saving artifacts...
Running migration: 2_deploy_contracts.js
  Deploying ECTools...
  ... 0x5c30582fa97fd591b561a4420d22c479830d9b8fbcfd043a5043fd391391611c
  ECTools: 0x14a626a41374c3ad11dba2f5b69af2544574521d
  Linking ECTools to ChannelManager
  Deploying ChannelManager...
  ... 0x64570184688bde8ad5fa6ba297ba29cd83aa6ea029d797d804aeaa1f5c4ef75c
  ChannelManager: 0x58504f635a76fbf45822c58abf8a64df6573378f
  Deploying SimpleToken...
  ... 0xc1a5d789f4a4e370eeeb4c9329558411e2c7281e53a48bbf689f2aa28fe7b624
  SimpleToken: 0xd09dfde368305517289c68778a3b78dfea49e4c1
Saving successful migration to network...
  ... 0x8b2f3dad71701cefc26be141092f80bd9fe8b0bcefc86b9e324b6089f5e5fa07
  ... 0xc1dbdefb7d13dcb668f3905d3246eb932eedc3796c5b40075cdc8879e4355ad3
Saving artifacts...
```

To retrieve contract address from the built artifacts:

```sh
export CONTRACT_ADDRESS=$(jq -r '.networks| to_entries | sort_by(.key) | last.value.address' build/contracts/ChannelManager.json)
```

Now you can check out this PR and execute tests:

```sh
cd ~/guac_rs
cargo test
```

WIP.
