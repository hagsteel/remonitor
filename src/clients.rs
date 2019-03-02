use std::io::{Read, Write};
use std::marker::PhantomData;

use bytes::Bytes;
use sonr::errors::Result;
use sonr::reactor::{Reaction, Reactive};
use sonr::sync::signal::{ReactiveSignalReceiver, SignalReceiver};
use sonr::{Event, Evented, Token};

use crate::codecs::Codec;
use crate::connections::{status_msg, Connection, Connections, StreamRef};

// -----------------------------------------------------------------------------
// 		- Client container -
// -----------------------------------------------------------------------------
pub struct Clients<S, T, C>
where
    S: StreamRef<T> + Read + Write,
    T: Evented + Read + Write,
    C: Codec,
{
    receiver: ReactiveSignalReceiver<Bytes>,
    connections: Connections<S, T, C>,
    _p: PhantomData<T>,
}

impl<S, T, C> Clients<S, T, C>
where
    S: StreamRef<T> + Read + Write,
    T: Evented + Read + Write,
    C: Codec,
{
    pub fn new(receiver: SignalReceiver<Bytes>) -> Result<Self> {
        Ok(Self {
            receiver: ReactiveSignalReceiver::new(receiver)?,
            connections: Connections::new(),
            _p: PhantomData,
        })
    }

    fn receive_messages(&mut self, token: Token) -> bool {
        if token != self.receiver.token() {
            return false;
        }

        while let Ok(message) = self.receiver.try_recv() {
            for (connection, _) in self.connections.connections.values_mut() {
                connection.push_write_buffer(message.clone());
                while let Some(Ok(_n)) = connection.write_buffer() {}
            }
        }

        true
    }
}

impl<S, T, C> Reactive for Clients<S, T, C>
where
    S: StreamRef<T> + Read + Write,
    T: Evented + Read + Write,
    C: Codec,
{
    type Input = (Connection<S, T>, C);
    type Output = ();

    fn reacting(&mut self, event: Event) -> bool {
        if self.receive_messages(event.token()) {
            return true;
        }

        self.connections.write_messages(event)
    }

    // Accept a new connection.
    fn react_to(&mut self, value: Self::Input) {
        let (mut connection, codec) = value;
        let buf = status_msg("OK");
        let bytes = C::encode(buf);
        connection.push_write_buffer(bytes);
        if connection.writable() {
            while let Some(Ok(_)) = connection.write_buffer() { }
        }
        self.connections
            .connections
            .insert(connection.token(), (connection, codec));
    }

    // There is no output
    fn react(&mut self) -> Reaction<Self::Output> {
        Reaction::NoReaction
    }
}
