use std::fmt::Debug;
use std::collections::VecDeque;
use std::io::Read;
use serde::de::DeserializeOwned;
use serde::Serialize;

use bytes::Bytes;

mod newline;

pub use newline::LineCodec;
