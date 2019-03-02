use std::io::{Read, Write};

use bytes::Bytes;
use sonr::reactor::{Reaction, Reactive};
use sonr::sync::broadcast::Broadcast;
use sonr::{Event, Evented};

use crate::codecs::Codec;
use crate::connections::{Message, status_msg};
use crate::connections::{Connection, Connections, StreamRef};

// -----------------------------------------------------------------------------
// 		- Monitor container -
// -----------------------------------------------------------------------------
pub struct Monitors<S: StreamRef<T>, T: Evented + Read + Write, C>
where
    S: StreamRef<T> + Read + Write,
    T: Evented + Read + Write,
    C: Codec,
{
    connections: Connections<S, T, C>,
    broadcast: Broadcast<Bytes>,
}

impl<S, T, C> Monitors<S, T, C>
where
    S: StreamRef<T> + Read + Write,
    T: Evented + Read + Write,
    C: Codec,
{
    pub fn new(broadcast: Broadcast<Bytes>) -> Self {
        Self {
            connections: Connections::new(),
            broadcast,
        }
    }
}

impl<S, T, C> Reactive for Monitors<S, T, C>
where
    S: StreamRef<T> + Read + Write,
    T: Evented + Read + Write,
    C: Codec,
{
    type Input = (Connection<S, T>, C);
    type Output = ();

    fn reacting(&mut self, event: Event) -> bool {
        let broadcast = &self.broadcast;

        let r = self
            .connections
            .read_messages::<_, Message>(event, |message| { 
                broadcast.publish(C::encode(message))
            });

        let w = self.connections.write_messages(event);

        w | r
    }

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

    fn react(&mut self) -> Reaction<Self::Output> {
        Reaction::NoReaction
    }
}
