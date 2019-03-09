use std::marker::PhantomData;
use std::sync::Arc;
use std::io::{Read, Write};
use std::collections::HashMap;
use std::time::{Instant, Duration};

use log::{warn, error};
use bytes::Bytes;
use sonr::Evented;
use sonr::prelude::*;
use sonr::reactor::{Reaction, Reactor};
use sonr::errors::Result;
use sonr::sync::signal::SignalSender;

use crate::codecs::Codec;
use crate::connections::{Connection, StreamRef, ConnectionState};
use crate::config::Config;
use crate::throttle::{Throttle, ThrottleKey};

mod message;
use message::AuthMessage;

#[derive(Debug)]
enum AuthState {
    NotAuthenticated,
    ClientId(Vec<u8>),
    Authenticated,
    Throttled(Instant, Duration),
}

const THROTTLE_TIME_SEC: u64 = 10;

impl AuthState {
    fn authenticate(&self, data: Bytes, config: &Config) -> Result<AuthState> {
        use AuthState::*;
        let state = match self {
            NotAuthenticated => ClientId(data.to_vec()),
            ClientId(id) => { 
                let id = String::from_utf8(id.to_vec())?;
                match config.auth.get(&id).map(|secret| secret.as_bytes() == data.as_ref()) {
                    Some(true) => Authenticated,
                    _ => Throttled(Instant::now(), Duration::from_secs(THROTTLE_TIME_SEC))
                } 
            }
            Throttled(instant, duration) => { 
                match instant.elapsed() > *duration {
                    true => ClientId(data.to_vec()),
                    false => Throttled(*instant, *duration)
                }
            }
            Authenticated => {
                error!("Client was already authenticated");
                panic!("----> this is not right")
            }
        };

        Ok(state)
    }
}

pub struct Authentication<S, T, C>
where
    S: StreamRef<T> + Read + Write,
    T: Evented + Read + Write,
    C: Codec,
{
    connections: HashMap<Token, (Connection<S, T>, C, AuthState)>,
    authenticated_connections: Vec<(Connection<S, T>, C)>,
    config: Arc<Config>,
    _p: PhantomData<T>,
    throttle_tx: Option<SignalSender<(String, Throttle)>>,
}

impl<S, T, C> Authentication<S, T, C> 
where
    S: StreamRef<T> + Read + Write,
    T: Evented + Read + Write,
    C: Codec,
{ 
    pub fn new(config: Arc<Config>, throttle_tx: Option<SignalSender<(String, Throttle)>>) -> Self {
        Self { 
            connections: HashMap::new(),
            authenticated_connections: Vec::new(),
            config,
            _p: PhantomData,
            throttle_tx,
        }
    }
} 

impl<S, T, C> Reactor for Authentication<S, T, C>
where
    S: StreamRef<T> + Read + Write,
    T: Evented + Read + Write + ThrottleKey,
    C: Codec,
{
    type Input = S;
    type Output = (Connection<S, T>, C);

    fn react(&mut self, reaction: Reaction<Self::Input>) -> Reaction<Self::Output> {
        match reaction {
            Reaction::Value(stream) => {
                let codec = C::default();
                let connection = Connection::new(stream);
                self.connections.insert(connection.token(), (connection, codec, AuthState::NotAuthenticated));
            }

            Reaction::Event(event) => {
                let config = self.config.clone();
                if let Some((connection, codec, state)) = self.connections.get_mut(&event.token()) {
                    if !connection.reacting(event) { return true; }

                    if let ConnectionState::Open = connection.state {
                        while connection.readable() {
                            let _res = codec.decode(connection); 
                            let mut messages = codec.drain::<AuthMessage>();
                            while let Some(Ok(message)) = messages.pop_front() {
                                match state.authenticate(message.payload, &config) {
                                    Ok(new_state) => *state = new_state,
                                    Err(_) => {}
                                }

                                if let AuthState::Throttled(i, d) = state {
                                    if let Ok(key) = connection.get_throttle_key() {
                                        match &self.throttle_tx {
                                            Some(tx) => { tx.send((key, Throttle::new(i.clone(), d.clone()))); }
                                            None => {}
                                        }
                                    }
                                    self.connections.remove(&event.token());
                                    return true
                                }

                                if let AuthState::Authenticated = state {
                                    if let Some((connection, codec, _)) = self.connections.remove(&event.token()) {
                                        self.authenticated_connections.push((connection, codec));
                                    }
                                    return true
                                }
                            }
                        }


                        if connection.writable() && connection.buffer_count() > 0 {
                            while let Some(Ok(_)) = connection.write_buffer() { }
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
        }

    }
}
