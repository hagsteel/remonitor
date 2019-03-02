use serde::Deserialize;
use bytes::Bytes;

#[derive(Debug, Deserialize)]
pub struct AuthMessage {
    pub payload: Bytes,
}
