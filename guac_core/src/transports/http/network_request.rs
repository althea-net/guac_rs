use clarity::{Address, Signature};
use crypto::CryptoService;
use serde::{de::Deserializer, ser::Serializer, Deserialize};
use std::str::FromStr;
use CRYPTO;

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

fn ser_signature_as_str<S>(x: &Signature, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&x.to_string())
}

fn de_signature_as_str<'de, D>(d: D) -> Result<Signature, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(d)
        .and_then(move |val| Signature::from_str(&val).map_err(serde::de::Error::custom))
}

/// A wrapper type that serializes a signature to/from a string.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignatureDef(
    #[serde(
        serialize_with = "ser_signature_as_str",
        deserialize_with = "de_signature_as_str"
    )]
    pub Signature,
);

impl Into<Signature> for SignatureDef {
    fn into(self) -> Signature {
        self.0
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SendProposalResponse {
    pub signature: SignatureDef,
}
