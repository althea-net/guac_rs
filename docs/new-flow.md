Guac manages channel payments between Althea nodes. It has only a few main points of integration with the rest of the system, which we call the "user api". When we refer to the "user" in this document, we are referring to the user of Guac, which is likely another automated daemon, not a person.

- Register: This allows the user to add information about a new counterparty that they would possibly like to pay.
- Fill channel: This allows the user to request that a channel be opened (or refilled) to pay a counterparty. This results in some money being locked up, and a transaction fee from the blockchain, and so must be used with discretion.
- Make payment: This allows the user to pay a counterparty who has previously had a channel filled. The payment cannot be larger than the amount of money available in the channel.
- Withdraw: This allows the user to take money out of a particular channel and return it to the user's normal blokchain account where it can be filled into another channel or transfered.

## Flow

We call `Register` and supply the information of a new counterparty. This includes the counterparty's ethereum address and its network address.

## Opening from scratch

When we decide that it would like to lock up some money to pay the counterparty we call `FillChannel`. This includes `fill_amount`, how much money we would like to lock up in a channel to the counterparty.

`FillChannel` will do different things based on whether or not there is a channel open already with the counterparty. In this case, since the counterparty was just added, there is no channel open.

Guac calls `ProposeChannel` on the counterparty with the amount that it wants to lock on our side of the channel. The counterparty responds with a signed `newChannel` transaction to open the channel, setting a balance of 0 on the counterparty's side and the specified `fill_amount` on our side. The counterparty also goes into the `OtherCreating` state. If this transaction is valid, Guac signs it, submits it to the blockchain, and goes into the `Creating` state.

The `Creating` and `OtherCreating` states signify that the `newChannel` transaction has been submitted to the blockchain but has not been confirmed yet. During these states, Guac will not sign another `newChannel` transaction or sign or accept any state updates to the channel. The `newChannel` transaction includes an expiration time. The `Creating` and `OtherCreating` states will transition back to `New` when this time has elapsed.

But hopefully, the `newChannel` transaction will have been confirmed before it expires. To find out when the confirmation has happened, the node in the `Creating` state (this is us) listens for the `channelOpened` event from the Guac contract on the blockchain. When we recieve this event we call `ChannelOpened` on the counterparty. The counterparty checks the blockchain to see whether it really has been opened and goes into the `Open` state. When we receive the response from the `ChannelOpened` endpoint, we go into the `Open` state as well. Guac nodes in the `Open` state are able to send and accept channel updates.
