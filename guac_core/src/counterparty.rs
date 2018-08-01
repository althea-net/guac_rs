use althea_types::EthAddress;

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Counterparty {
    pub address: EthAddress,
    // assuming ipv6 socketaddr
    pub url: String,
}

impl Counterparty {}
