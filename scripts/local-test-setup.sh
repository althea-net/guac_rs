#!/bin/bash
command -v ganache-cli >/dev/null 2>&1 || { echo >&2 "I require ganache-cli but it's not installed.  Aborting."; exit 1; }
command -v jq >/dev/null 2>&1 || { echo >&2 "I require jq but it's not installed.  Aborting."; exit 1; }

[ ! -d ".contract" ] && \
    git clone https://github.com/althea-mesh/guac .contract && \
    npm install .contract

ganache-cli -u 0 -u 1 -u 2 -m 'cook mango twist then skin sort option civil have still rather guilt' > /dev/null &

pushd .contract
truffle compile
truffle migrate --verbose
popd

CHANNEL_ADDRESS=`jq -r '.networks| to_entries | sort_by(.key) | last.value.address' .contract/build/contracts/PaymentChannels.json`

echo "contract_address = \"$CHANNEL_ADDRESS\"
private_key_0 = \"86de2cf259bf21a9aa2b8cf78f89ed479681001ca320c5762bb3237db65445cb\"
private_key_1 = \"06e744bba37fd1e630dc775d10fd8cbe0b5643f4d7187072d3d08df4b4118acf\"" > guac_http/config.toml