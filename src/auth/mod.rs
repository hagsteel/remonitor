use std::io::{Read, Write};
use std::marker::PhantomData;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use log::{error, info};
use sonr::errors::Result as SonrResult;
use sonr::prelude::*;
use sonr::Evented;
use sonr::net::stream::StreamRef;
use sonr::reactor::{Reaction, Reactor};
use sonr::sync::signal::SignalSender;

use crate::config::Config;
use crate::throttle::{Throttle, ThrottleKey};
use sonr_connection::{Codec, Connection};

mod message;
pub use message::AuthMessage;

#[derive(Debug)]
enum AuthState {
    NotAuthenticated,
    ClientId(Vec<u8>),
    Authenticated,
    Throttled(Instant, Duration),
}

const THROTTLE_TIME_SEC: u64 = 10;

impl AuthState {
    fn authenticate(&self, data: Bytes, config: &Config) -> SonrResult<AuthState> {
        use AuthState::*;
        let state = match self {
            NotAuthenticated => ClientId(data.to_vec()),
            ClientId(id) => {
                let id = String::from_utf8(id.to_vec())?;
                match config
                    .auth
                    .get(&id)
                    .map(|secret| secret.as_bytes() == data.as_ref())
                {
                    Some(true) => Authenticated,
                    _ => Throttled(Instant::now(), Duration::from_secs(THROTTLE_TIME_SEC)),
                }
            }
            Throttled(instant, duration) => match instant.elapsed() > *duration {
                true => ClientId(data.to_vec()),
                false => Throttled(*instant, *duration),
            },
            Authenticated => {
                error!("Client was already authenticated");
                panic!("----> this should not happen")
            }
        };

        Ok(state)
    }
}

pub struct Authentication<T, C, S>
where
    T: StreamRef<Evented=S> + Read + Write,
    C: Codec<Message = AuthMessage>,
    S: Evented + Read + Write + ThrottleKey,
{
    connections: HashMap<Token, (Connection<T, C>, AuthState)>,
    config: Arc<Config>,
    throttle_tx: Option<SignalSender<(String, Throttle)>>,
    _p: PhantomData<S>,
}

impl<T, C, S> Authentication<T, C, S>
where
    T: StreamRef<Evented=S> + Read + Write,
    C: Codec<Message = AuthMessage>,
    S: Evented + Read + Write + ThrottleKey,
{
    pub fn new(config: Arc<Config>, throttle_tx: Option<SignalSender<(String, Throttle)>>) -> Self {
        Self {
            connections: HashMap::new(),
            config,
            throttle_tx,
            _p: PhantomData,
        }
    }
}

impl<T, C, S> Reactor for Authentication<T, C, S>
where
    T: StreamRef<Evented=S> + Read + Write,
    C: Codec<Message = AuthMessage>,
    S: Evented + Read + Write + ThrottleKey,
{
    type Input = T;
    type Output = T;

    fn react(&mut self, reaction: Reaction<Self::Input>) -> Reaction<Self::Output> {
        match reaction {
            Reaction::Value(stream) => {
                let codec = C::default();
                let connection = Connection::new(stream, codec);
                self.connections.insert(
                    connection.token(),
                    (connection, AuthState::NotAuthenticated),
                );
                Reaction::Continue
            }

            Reaction::Event(event) => {
                if let Some((connection, state)) = self.connections.get_mut(&event.token()) {
                    let config = self.config.clone();
                    let mut vals = VecDeque::new();
                    let reacto = connection.react(event.into());
                    match reacto {
                        Reaction::Value(val) => {
                            vals.push_back(val);
                            while let Reaction::Value(val) = connection.react(Reaction::Continue) {
                                vals.push_back(val);
                            }
                        }
                        _ => {}
                    }

                    for val in vals {
                        match val {
                            Ok(msg) => match state.authenticate(msg.payload, &config) {
                                Ok(new_state) => *state = new_state,
                                Err(_) => {}
                            },
                            Err(e) => {
                                dbg!(e);
                                self.connections.remove(&event.token());
                                return Reaction::Continue;
                            }
                        }

                        match state {
                            AuthState::Throttled(i, d) => {
                                if let Ok(key) = connection.stream_ref().inner().get_throttle_key() {
                                    info!("Throttled  {:?}", connection.stream_ref().inner().get_throttle_key());
                                    if let Some(tx) =  &self.throttle_tx {
                                        let _ = tx.send((key, Throttle::new(i.clone(), d.clone())));
                                    }
                                }
                                self.connections.remove(&event.token());
                                return Reaction::Continue
                            }
                            AuthState::Authenticated => {
                                match self.connections.remove(&event.token()) {
                                    Some((connection, _state)) => {
                                        info!("Authenticated {:?}", connection.stream_ref().inner().get_throttle_key());
                                        return Reaction::Value(connection.into_inner());
                                    }
                                    None => return Reaction::Continue
                                }
                            }
                            _ => {}
                        }
                    }
                    Reaction::Continue
                } else {
                    event.into()
                }
            }
            Reaction::Continue => Reaction::Continue,
        }
    }
}
