use althea_types::EthAddress;

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Counterparty {
    pub address: EthAddress,
    pub url: String,
}

impl Counterparty {}
