# This file documents the requests which guac makes to contacts another guac node to start a channel

(actual endpoint names, ports, types WIP)

## Propose Channel
Tells a neighbor if you would like to accept a channel from them

Request data type: `Channel` (maybe a different type, but basically all the info you need to call the
`openChannel` contract call. Important values to check are the challenge period and the token
contract (if using erc20 tokens))

return type: `bool` accept or reject (maybe a enum in the future for different rejection reasons)

## Join Channel
Join the channel of a neighbor who has previously proposed a channel to you

Request data type: `Channel`

return data type: `bool` for acknowledgement
