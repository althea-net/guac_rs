use clarity::Address;

#[derive(Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Counterparty {
    pub address: Address,
    // assuming ipv6 socketaddr
    pub url: String,
}

impl Counterparty {}
