pub mod messages;

// use std::collections::{HashMap, VecDeque};
// use std::fmt::Debug;
// use std::io::{self, ErrorKind::WouldBlock, Read, Write};
// use std::marker::PhantomData;
// 
// use bytes::Bytes;
// use serde::de::DeserializeOwned;
// use sonr::errors::Result;
// use sonr::net::stream::Stream;
// use sonr::reactor::{Reaction, Reactor};
// use sonr::{Event, Evented, Token};
// use sonr_connection::{Connection, Codec};
// 
// use crate::throttle::ThrottleKey;
// 
// use messages::Message;
// 
// pub trait StreamRef<T: Evented + Read + Write> {
//     fn stream(&self) -> &Stream<T>;
//     fn stream_mut(&mut self) -> &mut Stream<T>;
// }
// 
// impl<T: Evented + Read + Write> StreamRef<T> for Stream<T> {
//     fn stream(&self) -> &Stream<T> {
//         self
//     }
// 
//     fn stream_mut(&mut self) -> &mut Stream<T> {
//         self
//     }
// }
// 
// #[derive(Debug)]
// pub enum ConnectionState {
//     Open,
//     Blocked,
//     Closed,
// }
// 
// impl<S, T, C> ThrottleKey for Connection<S, T, C>
// where
//     S: StreamRef<T> + Read + Write,
//     T: Evented + Read + Write + ThrottleKey,
//     C: Codec,
// {
//     fn get_throttle_key(&self) -> Result<String> {
//         let s = self.inner.stream();
//         s.inner().get_throttle_key()
//     }
// }
// 
// // -----------------------------------------------------------------------------
// // 		- Connections -
// // -----------------------------------------------------------------------------
// pub struct Connections<S, T, C>
// where
//     S: StreamRef<T> + Read + Write,
//     T: Evented + Read + Write,
//     C: Codec,
// {
//     pub connections: HashMap<Token, Connection<S, T, C>>,
//     _p: PhantomData<T>,
// }
// 
// impl<S, T, C> Connections<S, T, C>
// where
//     S: StreamRef<T> + Read + Write,
//     T: Evented + Read + Write,
//     C: Codec,
// {
//     pub fn new() -> Self {
//         Self {
//             connections: HashMap::new(),
//             _p: PhantomData,
//         }
//     }
// 
//     pub fn read_messages<F, M>(&mut self, event: Event, callback: F) -> bool
//     where
//         M: Debug + DeserializeOwned,
//         F: Fn(M),
//     {
//         if let Some((connection, codec)) = self.connections.get_mut(&event.token()) {
//             // Event was not consumed by the connection
//             if let Reaction::Event(_) = connection.react(event) {
//                 return true;
//             }
// 
//             while connection.readable() {
//                 codec.decode(connection);
//                 let messages = codec.drain::<M>();
//                 for message in messages {
//                     match message {
//                         Ok(message) => callback(message),
//                         Err(e) => connection.push_write_buffer(C::encode(e)),
//                     }
//                 }
//             }
// 
//             if let ConnectionState::Closed = connection.state {
//                 self.connections.remove(&event.token());
//             }
// 
//             true
//         } else {
//             false
//         }
//     }
// 
//     pub fn write_messages(&mut self, event: Event) -> bool {
//         if let Some((connection, _codec)) = self.connections.get_mut(&event.token()) {
//             if !connection.reacting(event) {
//                 return true;
//             }
// 
//             if connection.writable() {
//                 while let Some(Ok(_)) = connection.write_buffer() {}
//             }
// 
//             if let ConnectionState::Closed = connection.state {
//                 self.connections.remove(&event.token());
//             }
// 
//             true
//         } else {
//             false
//         }
//     }
// }
