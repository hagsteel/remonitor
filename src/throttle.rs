use std::collections::HashMap;
use std::time::{Duration, Instant};

use sonr::errors::Result;
use sonr::net::tcp::TcpStream;
use sonr::net::uds::UnixStream;
use sonr::prelude::*;
use sonr::sync::queue::ReactiveQueue;
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

impl ThrottleKey for UnixStream {
    fn get_throttle_key(&self) -> Result<String> {
        let addr = self.peer_addr()?;
        eprintln!("{:?}", addr);
        Ok(format!("{:?}", addr))
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
    inner: ReactiveQueue<T>,
    throttle_check: usize,
}

impl<T> ThrottledOutput<T> {
    pub fn new(inner: ReactiveQueue<T>) -> Result<Self> {
        Ok(Self {
            throttle_rx: ReactiveSignalReceiver::new(Capacity::Unbounded.into())?,
            throttled: HashMap::new(),
            inner,
            throttle_check: THROTTLE_CHECK,
        })
    }

    pub fn get_mut(&mut self) -> &mut ReactiveQueue<T> {
        &mut self.inner
    }

    pub fn sender(&self) -> SignalSender<(String, Throttle)> {
        self.throttle_rx.sender()
    }
}

impl<T> Reactive for ThrottledOutput<T>
where
    T: ThrottleKey + Send + 'static,
{
    type Output = ();
    type Input = T;

    fn reacting(&mut self, event: Event) -> bool {
        if self.throttle_rx.reacting(event) {
            if let Ok((addr, throttle)) = self.throttle_rx.try_recv() {
                self.throttled.insert(addr, throttle);
            }
            true
        } else {
            self.inner.reacting(event)
        }
    }

    fn react_to(&mut self, value: Self::Input) {
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
                Some(false) => {}
                Some(true) => {
                    self.throttled.remove(&key);
                    self.inner.react_to(value);
                }
                None => self.inner.react_to(value),
            }
        } else {
            self.inner.react_to(value);
        }
    }

    fn react(&mut self) -> Reaction<Self::Output> {
        self.inner.react()
    }
}
