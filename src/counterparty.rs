use althea_types::EthAddress;

#[derive(Serialize, Deserialize, Clone)]
pub struct Counterparty {
    pub address: EthAddress,
    pub url: String,
}

impl Counterparty {}
