use crate::{Error, ResponseError};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub fn decode<T: DeserializeOwned>(token: &str, field: &'static str) -> Result<T, Error> {
    let bytes = URL_SAFE_NO_PAD
        .decode(token)
        .map_err(|_| Error::InvalidRequest { field })?;
    serde_json::from_slice(&bytes).map_err(|_| Error::InvalidRequest { field })
}

pub fn encode<T: Serialize>(value: &T) -> Result<String, Error> {
    let bytes = serde_json::to_vec(value).map_err(|_| ResponseError::InvalidOperationResponse)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
