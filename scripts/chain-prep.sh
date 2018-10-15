#!/bin/bash
set -eux
killall node 2>/dev/null || true
ganache-cli -u 0 -u 1 -u 2 -m 'cook mango twist then skin sort option civil have still rather guilt' &
git clone https://github.com/althea-mesh/simple-bidirectional-erc20-channel contract
pushd contract 
npm install .
truffle compile
addresses="$(truffle migrate --verbose)"
export CHANNEL_ADDRES=`jq -r '.networks| to_entries | sort_by(.key) | last.value.address' build/contracts/ChannelManager.json`
export CHANNEL_ADDRES=`jq -r '.networks| to_entries | sort_by(.key) | last.value.address' build/contracts/SimpleToken.json`
popd
