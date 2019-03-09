use std::io::{Read, BufReader};
use std::fs::File;
use std::collections::HashMap;
use sonr::reactor::{Reactor, Reaction};
use sonr::errors::Result;
use serde_derive::Deserialize;

#[derive(Clone, Deserialize, Debug)]
pub struct Config {
    pub auth: HashMap<String, String>,
    uds_monitor_path: Option<String>,
    uds_client_path: Option<String>,
    tcp_monitor_host: Option<String>,
    tcp_client_host: Option<String>,
    pub pfx_cert_path: String,
    pub pfx_pass: String,
    pub thread_count: usize,
    pub enable_log: bool,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self> {
        let mut buf = Vec::new();
        let file = File::open(path)?;
        let mut br = BufReader::new(file);
        br.read_to_end(&mut buf);
        Ok(toml::from_slice(&buf).unwrap())
    }

    pub fn use_uds(&self) -> bool {
        self.uds_monitor_path.is_some() && self.uds_client_path.is_some()
    }

    pub fn uds_client_path(&self) -> &str {
        self.uds_client_path.as_ref().unwrap()
    }

    pub fn uds_monitor_path(&self) -> &str {
        self.uds_monitor_path.as_ref().unwrap()
    }

    pub fn use_tcp(&self) -> bool {
        self.tcp_monitor_host.is_some() && self.tcp_client_host.is_some()
    }

    pub fn tcp_client_host(&self) -> &str {
        self.tcp_client_host.as_ref().unwrap()
    }

    pub fn tcp_monitor_host(&self) -> &str {
        self.tcp_monitor_host.as_ref().unwrap()
    }
}

pub struct Optional<T: Reactor> {
    reactor: Option<T>,
}

impl<T: Reactor> Optional<T> {
    pub fn new<F>(enabled: bool, f: F) -> Self 
        where F: FnOnce() -> T
    {
        let reactor = if enabled { Some(f()) } else { None };
        Self { 
            reactor
        }
    }
}

impl<T: Reactor> Reactor for Optional<T> {
    type Output = T::Output;
    type Input = T::Input;

    fn react(&mut self, reaction: Reaction<Self::Input>) -> Reaction<Self::Output> {
        match self.reactor {
            Some(ref mut r) => r.react(reaction),
            None => Reaction::Continue 
        }
    }
}
