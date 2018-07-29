# This file documents the requests which guac makes to contacts another guac node

## Propose Channel
Tells a neighbor if you would like to accept a channel from them

Endpoint: /propose

Request data type: `Channel` (basically all the info you need to call the
`openChannel` contract call. Important values to check are the challenge period and the token
contract (if using erc20 tokens))

return type: `bool` accept or reject (maybe a enum in the future for different rejection reasons)

## Update
Tells your counterparty about your new state

Endpoint: /update

Request data type: `UpdateTx`
Return data type: `UpdateTx` (containing rebased transaction data from their local state)

## Channel Created
Tells your neighbor who has previously accepted your proposal that the your channel creation transaction
has been confirmed

Request data type: `Channel`

return data type: `null`

## Channel Joined
Tells your neighbor who has a channel open with you that your channel join transaction has been confirmed 

Request data type: `Channel`

return data type: `null`