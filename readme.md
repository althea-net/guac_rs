# Guac light client

Guac is an Ethereum single hop payment channel client for the Ethcalate Bi-Directional Payment
Channel. This light client is able to send and verify channel opening, updating, and ending
transactions. It relies on one or several bounty hunting nodes full nodes which relay transactions
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