use std::marker::PhantomData;
use std::collections::VecDeque;
use std::io::{Read, ErrorKind::WouldBlock};

use serde::de::DeserializeOwned;
use serde::Serialize;
use bytes::{Bytes, BytesMut};
use sonr_connection::codec::{Codec, Decoding, CodecError};

pub struct LineCodec<T> {
    buffer: BytesMut,
    lines: Vec<Bytes>,
    _p: PhantomData<T>,
}

impl<T> Default for LineCodec<T> {
    fn default() -> Self {
        let mut buffer = BytesMut::with_capacity(1024);
        unsafe { buffer.set_len(1024); }
        Self { 
            buffer,
            lines: Vec::new(),
            _p: PhantomData,
        }
    } 
}

impl<T: DeserializeOwned> Codec for LineCodec<T> {
    type Message = T;

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

    fn drain(&mut self) -> VecDeque<Result<T, CodecError>> {
        let mut vec = VecDeque::new();
        for line in self.lines.drain(..).into_iter() {
            let res = serde_json::from_slice(&line);
            match res {
                Ok(val) => vec.push_back(Ok(val)),
                Err(_) => vec.push_back(Err(CodecError::MalformedMessage(line)))
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
