# Initialization (see also guac-external-api/init.md)

When a rita node learns about rita another node, it sends a Register message to guac (either HTTP or actix),
which contains the `counterparty` struct for that node. This is then added to storage.

When a node wants to create a channel to another node, it checks to make sure it has the
counterparty struct (importantly the url) for that node, then according to it's own configuration,
creates `Channel` (mostly empty) with its proposed channel with the other node. Then it sends it to the Propose
Channel endpoint. If the other node is happy with the channel, it will return true and the first
node will create a blockchain transaction opening the channel. When that is complete, it will send
a JoinChannel message containing a filled out `Channel` struct, and the second node will also make
a transaction to join the channel.

However what happens when 2 nodes send the Propose Channel message at the same time? A simple rule
is specified, a node will accept a proposal even if it has a pending proposal if it has the numerically
larger ethereum address. This guarantees that at least one node will be successful in the proposal process
if 2 nodes do it simultaneously.