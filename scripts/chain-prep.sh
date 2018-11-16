#!/bin/bash
killall node 2>/dev/null || true
ganache-cli -u 0 -u 1 -u 2 -m 'cook mango twist then skin sort option civil have still rather guilt' &
git clone https://github.com/althea-mesh/guac contract
pushd contract 
npm install .
truffle compile
truffle migrate --verbose
export CHANNEL_ADDRESS=`jq -r '.networks| to_entries | sort_by(.key) | last.value.address' build/contracts/PaymentChannels.json`
popd
