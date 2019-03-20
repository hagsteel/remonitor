use std::io::{Read, Write};
use std::collections::HashMap;

use bytes::Bytes;
use sonr::errors::Result;
use sonr::reactor::{Reaction, Reactor};
use sonr::sync::signal::{ReactiveSignalReceiver, SignalReceiver};
use sonr::net::stream::StreamRef;
use sonr::Token;

use sonr_connection::{Codec, Connection};
use crate::connections::messages::status_msg;

pub struct Clients<T, C>
where
    T: StreamRef + Read + Write,
    C: Codec,
{
    receiver: ReactiveSignalReceiver<Bytes>,
    connections: HashMap<Token, Connection<T, C>>,
}

impl<T, C> Clients<T, C>
where
    T: StreamRef + Read + Write,
    C: Codec,
{
    pub fn new(receiver: SignalReceiver<Bytes>) -> Result<Self> {
        Ok(Self {
            receiver: ReactiveSignalReceiver::new(receiver)?,
            connections: HashMap::new(),
        })
    }
}

impl<T, C> Reactor for Clients<T, C>
where
    T: StreamRef + Read + Write,
    C: Codec,
{
    type Input = T;
    type Output = ();

    fn react(&mut self, reaction: Reaction<Self::Input>) -> Reaction<Self::Output> {
        match reaction {
            Reaction::Event(event) => {
                if event.token() == self.receiver.token() {
                    while let Ok(message) = self.receiver.try_recv() {
                        for con in self.connections.values_mut() {
                            con.add_write_buffer(message.clone());
                            con.write_buffers();
                        }
                    }
                    return Reaction::Continue;
                }

                if let Some(con) = self.connections.get_mut(&event.token()) {
                    let _ = con.react(event.into());
                    Reaction::Continue
                } else {
                    Reaction::Event(event)
                }
            }
            Reaction::Value(stream) => { 
                let buf = status_msg("OK");
                let bytes = C::encode(buf);
                let mut connection = Connection::new(stream, C::default());
                connection.add_write_buffer(bytes);
                connection.write_buffers();
                self.connections.insert(connection.token(), connection); 
                Reaction::Continue
            }
            Reaction::Continue => Reaction::Continue,
        }
    }
}
