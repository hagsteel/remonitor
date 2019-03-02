use std::fmt::Debug;
use std::collections::VecDeque;
use std::io::Read;
use serde::de::DeserializeOwned;
use serde::Serialize;

use bytes::Bytes;

mod newline;
// mod length;

pub use newline::LineCodec;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodecError {
    MalformedMessage,
}


#[derive(Debug)]
pub enum Decoding {
    ConnectionError,
    Blocked,
    Succeeded
}

pub trait Codec : Default {
    fn decode(&mut self, readable: &mut impl Read) -> Decoding;
    fn drain<T: Debug + DeserializeOwned>(&mut self) -> VecDeque<Result<T, CodecError>>;
    fn encode(val: impl Serialize) -> Bytes;
}
