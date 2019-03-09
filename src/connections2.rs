use std::fmt::Debug;
use std::collections::{HashMap, VecDeque};
use std::io::{self, ErrorKind::WouldBlock, Read, Write};
use std::marker::PhantomData;

use bytes::Bytes;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sonr::errors::Result;
use sonr::net::stream::Stream;
use sonr::{Event, Evented, Token};
use sonr::reactor::Reaction;

use crate::codecs::Codec;
use crate::throttle::ThrottleKey;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    Error,
    Status
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Message {
    payload: String,
    message_type: MessageType,
}

pub fn status_msg(msg: &str) -> Message {
    Message { 
        payload: msg.to_owned(),
        message_type: MessageType::Status,
    }
}

pub trait StreamRef<T: Evented + Read + Write> {
    fn stream(&self) -> &Stream<T>;
    fn stream_mut(&mut self) -> &mut Stream<T>;
}

impl<T: Evented + Read + Write> StreamRef<T> for Stream<T> {
    fn stream(&self) -> &Stream<T> {
        self
    }

    fn stream_mut(&mut self) -> &mut Stream<T> {
        self
    }
}

#[derive(Debug)]
pub enum ConnectionState {
    Open,
    Blocked,
    Closed,
}

impl<S, T> ThrottleKey for Connection<S, T> 
where
    S: StreamRef<T> + Read + Write,
    T: Evented + Read + Write + ThrottleKey,
{
    fn get_throttle_key(&self) -> Result<String> {
        let s = self.inner.stream();
        s.inner().get_throttle_key()
    }
}

pub struct Connection<S, T>
where
    S: StreamRef<T> + Read + Write,
    T: Evented + Read + Write,
{
    inner: S,
    write_buffers: VecDeque<Bytes>,
    pub state: ConnectionState,
    _p: PhantomData<T>,
}

impl<S, T> Connection<S, T>
where
    S: StreamRef<T> + Read + Write,
    T: Evented + Read + Write,
{
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            write_buffers: VecDeque::new(),
            state: ConnectionState::Open,
            _p: PhantomData,
        }
    }

    pub fn push_write_buffer(&mut self, buf: Bytes) {
        self.write_buffers.push_back(buf);
    }

    pub fn buffer_count(&self) -> usize {
        self.write_buffers.len()
    }

    pub fn token(&self) -> Token {
        self.inner.stream().token()
    }

    // pub fn is_reacting(&mut self, event: Event) -> bool {
    //     match self.inner.stream_mut().react(Reaction::Event(event)) {
    //         false => false,
    //         true => {
    //             self.state = ConnectionState::Open;
    //             true
    //         }
    //     }
    // }

    pub fn readable(&self) -> bool {
        self.inner.stream().readable()
    }

    pub fn writable(&self) -> bool {
        self.inner.stream().writable()
    }

    pub fn write_buffer(&mut self) -> Option<io::Result<usize>> {
        match self.write_buffers.pop_front() {
            Some(buf) => {
                let res = self.inner.write(&buf);
                match res {
                    Ok(n) => {
                        self.write_buffers.remove(0);
                        if n != buf.len() {
                            let remainder = Bytes::from(&buf[n..]);
                            self.write_buffers.insert(0, remainder);
                        }
                    }
                    Err(ref e) if e.kind() == WouldBlock => self.state = ConnectionState::Blocked,
                    Err(ref _e) => self.state = ConnectionState::Closed,
                }
                Some(res)
            }
            None => None,
        }
    }
}

impl<S, T> Read for Connection<S, T>
where
    S: StreamRef<T> + Read + Write,
    T: Evented + Read + Write,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let res = self.inner.read(buf);

        match res {
            Err(ref e) if e.kind() == WouldBlock => self.state = ConnectionState::Blocked,
            Ok(0) => self.state = ConnectionState::Closed,
            Err(_) => self.state = ConnectionState::Closed,
            Ok(_) => {}
        }

        res
    }
}

// -----------------------------------------------------------------------------
// 		- Connections -
// -----------------------------------------------------------------------------
pub struct Connections<S, T, C>
where
    S: StreamRef<T> + Read + Write,
    T: Evented + Read + Write,
    C: Codec,
{
    pub connections: HashMap<Token, (Connection<S, T>, C)>,
    _p: PhantomData<T>,
}

impl<S, T, C> Connections<S, T, C>
where
    S: StreamRef<T> + Read + Write,
    T: Evented + Read + Write,
    C: Codec,
{
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            _p: PhantomData,
        }
    }

    pub fn read_messages<F, M>(&mut self, event: Event, callback: F) -> bool
    where
        M: Debug + DeserializeOwned,
        F: Fn(M),
    {
        if let Some((connection, codec)) = self.connections.get_mut(&event.token()) {
            // Event was not consumed by the connection
            if let Reaction::Event(_) = connection.react(event) { return true }

            while connection.readable() {
                codec.decode(connection);
                let messages = codec.drain::<M>();
                for message in messages {
                    match message {
                        Ok(message) => callback(message),
                        Err(e) => connection.push_write_buffer(C::encode(e)),
                    }
                }
            }

            if let ConnectionState::Closed = connection.state {
                self.connections.remove(&event.token());
            }

            true
        } else {
            false
        }
    }

    pub fn write_messages(&mut self, event: Event) -> bool {
        if let Some((connection, _codec)) = self.connections.get_mut(&event.token()) {
            if !connection.reacting(event) { return true } 

            if connection.writable() {
                while let Some(Ok(_)) = connection.write_buffer() { }
            }

            if let ConnectionState::Closed = connection.state {
                self.connections.remove(&event.token());
            }

            true
        } else {
            false
        }
    }
}
