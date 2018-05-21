use althea_types::EthAddress;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Counterparty {
    pub address: EthAddress,
    pub url: String,
}

impl Counterparty {}
