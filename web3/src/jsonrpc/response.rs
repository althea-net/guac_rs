use serde_json::Value;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Error<E> {
    pub code: i64,
    pub message: String,
    pub data: Option<E>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ResponseData<R, E> {
    Error { error: Error<E> },
    Success { result: R },
}

impl<R, E> ResponseData<R, E> {
    /// Consume response and return value
    pub fn into_result(self) -> Result<R, Error<E>> {
        match self {
            ResponseData::Success { result } => Ok(result),
            ResponseData::Error { error } => Err(error),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response<R, E = Value> {
    pub id: Value,
    pub jsonrpc: String,
    #[serde(flatten)]
    pub data: ResponseData<R, E>,
}

#[test]
fn test_easy_response() {
    let response: Response<u64> =
        serde_json::from_str(r#"{"jsonrpc": "2.0", "result": 19, "id": 1}"#).unwrap();
    assert_eq!(response.id.as_u64().unwrap(), 1);
    assert_eq!(response.data.into_result().unwrap(), 19);
}

#[test]
fn test_intermediate_response() {
    let response: Response<u64> =
        serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"result":"0x429d069189e0000"}"#).unwrap();
    assert_eq!(response.id.as_u64().unwrap(), 1);
    assert_eq!(response.data.into_result().unwrap(), 19);
}

#[test]
fn test_complex_response() {
    let response: Response<u64> =
        serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"result":{"blockHash":"0xb9497404e8cd1abc66c9823546b9151cb898a48c6af31afef3f43e61ae0ab580","blockNumber":"0x36d61b","from":"0x998dea6b32dc94f1b634897e75c83bcde1464339","gas":"0x5208","gasPrice":"0x3b9aca00","hash":"0x63259e15009ed9ca787c02b231e9a3aea8c06a1f8303c0bde09da4b3716f5b59","input":"0x","nonce":"0x2d","to":"0x72d9e579f691d62aa7e0703840db6dd2fa9fae21","transactionIndex":"0x6","value":"0x1485650ff3300","v":"0x2c","r":"0xf5e2ecbf8a259fb2434089b5c274a35e41097561fa16121934374ec4a013d65b","s":"0x158dc114877706401b3ecfb1deb4105a938f1c2ec8fe493fb061694577a84938"}}"#).unwrap();
    assert_eq!(response.id.as_u64().unwrap(), 1);
    assert_eq!(response.data.into_result().unwrap(), 19);
}

#[test]
fn test_error() {
    let response: Response<Value> = serde_json::from_str(r#"{"jsonrpc": "2.0", "error": {"code": -32601, "message": "Method not found"}, "id": "1"}"#).unwrap();
    assert_eq!(response.id.as_str().unwrap(), "1");
    let err = response.data.into_result().unwrap_err();
    assert_eq!(err.code, -32601);
    assert_eq!(err.message, "Method not found");
}
