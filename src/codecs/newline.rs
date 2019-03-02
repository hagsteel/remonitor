use std::fmt::Debug;
use std::collections::VecDeque;
use std::io::{Read, ErrorKind::WouldBlock};

use serde::de::DeserializeOwned;
use serde::Serialize;
use bytes::{Bytes, BytesMut};
use super::{Codec, Decoding, CodecError};

pub struct LineCodec {
    buffer: BytesMut,
    lines: Vec<Bytes>,
}

impl Default for LineCodec {
    fn default() -> Self {
        let mut buffer = BytesMut::with_capacity(1024);
        unsafe { buffer.set_len(1024); }
        Self { 
            buffer,
            lines: Vec::new(),
        }
    } 
}

impl Codec for LineCodec {
    fn decode(&mut self, readable: &mut impl Read) -> Decoding {
        let res = readable.read(&mut self.buffer[..]);

        match res {
            Ok(0) => Decoding::ConnectionError,
            Err(ref e) if e.kind() == WouldBlock => Decoding::Blocked,
            Err(_e) => Decoding::ConnectionError,
            Ok(mut n) => {
                while let Some(pos) = &self.buffer[..n].iter().position(|&b| b == b'\n') {
                    let line = self.buffer.split_to(*pos);
                    self.buffer.advance(1); // skip the newline char
                    self.lines.push(line.freeze());
                    n -= pos;
                }
                self.buffer.resize(1024, 0);
                Decoding::Succeeded
            }
        }
    }

    fn drain<T: Debug + DeserializeOwned>(&mut self) -> VecDeque<Result<T, CodecError>> {
        let mut vec = VecDeque::new();
        for line in self.lines.drain(..).into_iter() {
            let res = serde_json::from_slice(&line);
            match res {
                Ok(val) => vec.push_back(Ok(val)),
                Err(_) => vec.push_back(Err(CodecError::MalformedMessage))
            }
        }
        vec
    }

    fn encode(val: impl Serialize) -> Bytes {
        let mut bytes = serde_json::to_vec(&val).unwrap(); 
        bytes.push(b'\n');
        bytes.into()
    }
}
