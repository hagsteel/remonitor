use std::collections::HashMap;
use std::marker::PhantomData;
use std::time::{Duration, Instant};

use sonr_tls::TlsStream;

use sonr::errors::Result;
use sonr::net::tcp::TcpStream;
use sonr::prelude::*;
use sonr::sync::signal::{ReactiveSignalReceiver, SignalSender};
use sonr::sync::Capacity;

const MAX_THROTTLE: usize = 1024 * 8;
const THROTTLE_CHECK: usize = 20;

pub trait ThrottleKey {
    fn get_throttle_key(&self) -> Result<String>;
}

impl ThrottleKey for TcpStream {
    fn get_throttle_key(&self) -> Result<String> {
        let addr = self.peer_addr()?;
        Ok(addr.ip().to_string())
    }
}

impl ThrottleKey for TlsStream<Stream<TcpStream>> {
    fn get_throttle_key(&self) -> Result<String> {
        let addr = self.get_ref().inner().peer_addr()?;
        Ok(addr.ip().to_string())
    }
}

#[derive(Debug, Clone)]
pub struct Throttle {
    instant: Instant,
    duration: Duration,
}

impl Throttle {
    pub fn new(instant: Instant, duration: Duration) -> Self {
        Self { instant, duration }
    }

    fn expired(&self) -> bool {
        self.instant.elapsed() > self.duration
    }
}

pub struct ThrottledOutput<T> {
    throttle_rx: ReactiveSignalReceiver<(String, Throttle)>,
    throttled: HashMap<String, Throttle>,
    throttle_check: usize,
    _p1: PhantomData<T>,
}

impl<T> ThrottledOutput<T> {
    pub fn new() -> Result<Self> {
        Ok(Self {
            throttle_rx: ReactiveSignalReceiver::new(Capacity::Unbounded.into())?,
            throttled: HashMap::new(),
            throttle_check: THROTTLE_CHECK,
            _p1: PhantomData,
        })
    }

    pub fn sender(&self) -> SignalSender<(String, Throttle)> {
        self.throttle_rx.sender()
    }
}

impl<T> Reactor for ThrottledOutput<T>
where
    T: ThrottleKey + Send + 'static,
{
    type Output = T;
    type Input = T;

    fn react(&mut self, reaction: Reaction<Self::Input>) -> Reaction<Self::Output> {
        match reaction {
            Reaction::Value(value) => { 
                // Once THROTTLE_CHECK number of connections reached, 
                // cycle the connections and drop the expired ones
                //
                // This is a rather naive check, but it works for now
                self.throttle_check -= 1;
                if self.throttle_check == 0 {
                    self.throttle_check = THROTTLE_CHECK;
                    let mut remove_keys: Vec<String> = Vec::new();
                    for (k, v) in &self.throttled {
                        if v.expired() {
                            remove_keys.push(k.clone());
                        }
                    }
                    remove_keys.into_iter().for_each(|k| { self.throttled.remove(&k); });
                }

                if let Ok(key) = value.get_throttle_key() {
                    match self.throttled.get(&key).map(|t| t.expired()) {
                        Some(true) => {
                            self.throttled.remove(&key);
                            return Reaction::Value(value);
                        }
                        Some(_) | None => return Reaction::Value(value),
                    }
                } else {
                    return Reaction::Value(value);
                }
            }
            Reaction::Event(event) => {
                // Incoming throttles
                if self.throttle_rx.token() != event.token() {
                    return event.into();
                }

                if let Reaction::Value(val) = self.throttle_rx.react(event.into()) {
                    let (addr, throttle) = val;
                    self.throttled.insert(addr, throttle);
                }

                Reaction::Continue
            } 
            Reaction::Continue => Reaction::Continue,
        }
    }
}
