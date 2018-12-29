use clarity::Address;
// use crypto::CryptoService;
// use CRYPTO;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkRequest<T> {
    pub from_addr: Address,
    pub data: T,
}

impl<T> NetworkRequest<T> {
    pub fn from_data(data: T) -> NetworkRequest<T> {
        NetworkRequest {
            from_addr: CRYPTO.own_eth_addr(),
            data,
        }
    }
}
