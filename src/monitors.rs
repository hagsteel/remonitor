use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};

use log::error;
use bytes::Bytes;
use sonr::reactor::{Reaction, Reactor};
use sonr::sync::broadcast::Broadcast;
use sonr::net::stream::StreamRef;
use sonr::Token;
use sonr_connection::{Codec, Connection};

use crate::messages::{status_msg, Message};

pub struct Monitors<T, C>
where
    T: StreamRef + Read + Write,
    C: Codec,
{
    connections: HashMap<Token, Connection<T, C>>,
    broadcast: Broadcast<Bytes>,
}

impl<T, C> Monitors<T, C>
where
    T: StreamRef + Read + Write,
    C: Codec,
{
    pub fn new(broadcast: Broadcast<Bytes>) -> Self {
        Self {
            connections: HashMap::new(),
            broadcast,
        }
    }
}

impl<T, C> Reactor for Monitors<T, C>
where
    T: StreamRef + Read + Write,
    C: Codec<Message=Message>,
{
    type Input = T;
    type Output = ();

    fn react(&mut self, reaction: Reaction<Self::Input>) -> Reaction<Self::Output> {
        match reaction {
            Reaction::Event(event) => {
                let broadcast = &self.broadcast;
                if let Some(con) = self.connections.get_mut(&event.token()) {
                    let mut messages = VecDeque::new();
                    if let Reaction::Value(val) = con.react(event.into()) {
                        messages.push_back(val);
                        while let Reaction::Value(val) = con.react(Reaction::Continue) {
                            messages.push_back(val);
                        }
                    }

                    for message in messages {
                        match message {
                            Ok(msg) => {
                                broadcast.publish(C::encode(msg));
                            }
                            Err(e) => {
                                error!("{:?}", e);
                                // con.add_write_buffer(b"");
                                self.connections.remove(&event.token());
                                return Reaction::Continue
                            }
                        }
                    }
                    Reaction::Continue
                } else {
                    event.into()
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
