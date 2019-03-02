use std::collections::VecDeque;
use std::io::Read;
use std::io::ErrorKind::WouldBlock;
use bytes::{Bytes, BytesMut};

use super::{Codec, Message, Decoding};

pub struct LengthCodec {
    buffer: BytesMut,
    messages: Vec<Bytes>,
}

impl Default for LengthCodec {
    fn default() -> Self {
        let mut buffer = BytesMut::with_capacity(1024);
        unsafe { buffer.set_len(1024); }
        Self {
            buffer,
            messages: Vec::new(),
        }
    }
}


impl Codec for LengthCodec {
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
                    self.messages.push(line.freeze());
                    n -= pos;
                }
                Decoding::Succeeded
            }
        }
    }

    fn drain(&mut self) -> VecDeque<Message> {
        self.messages.drain(..).map(|line| Message::new(line)).collect()
    }
}
