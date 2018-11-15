Guac manages channel payments between Althea nodes. It has only a few main points of integration with the rest of the system, which we call the "user api". When we refer to the "user" in this document, we are referring to the user of Guac, which is likely another automated daemon, not a person.

- Register: This allows the user to add information about a new counterparty that they would possibly like to pay.
- Fill channel: This allows the user to request that a channel be opened (or refilled) to pay a counterparty. This results in some money being locked up, and a transaction fee from the blockchain, and so must be used with discretion.
- Make payment: This allows the user to pay a counterparty who has previously had a channel filled. The payment cannot be larger than the amount of money available in the channel.
- Withdraw: This allows the user to take money out of a particular channel and return it to the user's normal blokchain account where it can be filled into another channel or transfered.

## Flow

We call `Register` and supply the information of a new counterparty. This includes the counterparty's ethereum address and its network address.

## Opening from scratch

When we decide that it would like to lock up some money to pay the counterparty we call `Fill`. This includes `fill_amount`, how much money we would like to lock up in a channel to the counterparty.

`Fill` will do different things based on whether or not there is a channel open already with the counterparty. In this case, since the counterparty was just added, there is no channel open.

Guac calls `ProposeChannel` on the counterparty with the amount that it wants to lock on our side of the channel. The counterparty responds with a signed `newChannel` transaction to open the channel, setting a balance of 0 on the counterparty's side and the specified `fill_amount` on our side. The counterparty also goes into the `OtherCreating` state. If this transaction is valid, Guac signs it, submits it to the blockchain, and goes into the `Creating` state.

The `Creating` and `OtherCreating` states signify that the `newChannel` transaction has been submitted to the blockchain but has not been confirmed yet. During these states, Guac will not sign another `newChannel` transaction or sign or accept any state updates to the channel. The `newChannel` transaction includes an expiration time. The `Creating` and `OtherCreating` states will transition back to `New` when this time has elapsed.

But hopefully, the `newChannel` transaction will have been confirmed before it expires. To find out when the confirmation has happened, the node in the `Creating` state (this is us) listens for the `channelOpened` event from the Guac contract on the blockchain. When we recieve this event we call `ChannelOpened` on the counterparty. The counterparty checks the blockchain to see whether it really has been opened and goes into the `Open` state. When we receive the response from the `ChannelOpened` endpoint, we go into the `Open` state as well. Guac nodes in the `Open` state are able to send and accept channel updates.

## Refilling and withdrawing

When a Guac node wants to take money out of a channel or put money in, it uses this flow. Refilling and withdrawing are almost exactly the same operation. To start it off, we call the `Fill` or `Withdraw` endpoints, with the amount we would like to move.

Then our Guac calls the `Redraw` endpoint on the counterparty, with unsigned `NewChannel` and `CloseChannelFast` transactions. The `NewChannel` transaction contains our intended new balance for our side of the channel. The counterparty receives and signs these, goes into the `OtherCreating` state, and sends them back. We go into the `Creating` state and submit them. The `CloseChannelFast` transaction first closes the channel, giving the money back to the parties, then the `NewChannel` transaction opens a new channel with the new desired balances.

As in the case where we opened a channel from scratch, we wait for the `ChannelOpened` event from the blockchain and when receive it, call the counterparty's `ChannelOpened` endpoint. They go into the `Open` state, and when we recieve the successful response from them, we go into the `Open` state as well.

## Simultaneous opening

We address this case, since it is likely to happen that two nodes try to open channels with each other within a few seconds.

In this case, we open a channel first. We go through all the steps in the basic channel opening until we get to the `Creating` state. This is likely, since all these steps happen within milliseconds. It is most likely that the counterparty will try to open their channel while we are waiting for a response from the blockchain on the channel confirmation.

The counterparty calls `Fill` on their Guac while they are in the `OtherCreating` state. Their Guac goes through the same steps that it would when refilling an already open channel. It sends `NewChannel` and `CloseChannelFast` transactions with their side of the channel containing the balance they specified. When Our Guac recieves these, it goes into the `OtherCreating` state, signs them, and sends them back. The next steps are also the same as when refilling a channel.
