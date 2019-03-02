use std::thread;
use std::io::{Read, Write};
use std::fs::remove_file;
use std::sync::Arc;

use sonr::Evented;
use sonr::system::System;
use sonr::net::tcp::ReactiveTcpListener;
use sonr::net::uds::{ReactiveUdsListener, UnixStream};
use sonr::net::stream::Stream;
use sonr::reactor::Reactive;
use sonr::sync::queue::{ReactiveQueue, ReactiveDeque, Dequeue};
use sonr::sync::broadcast::Broadcast;
use sonr::sync::signal::{SignalSender, ReactiveSignalReceiver, SignalReceiver};
use sonr::errors::Result;
use native_tls::TlsStream;
use sonr_tls::ReactiveTlsAcceptor;

use crate::monitors::Monitors;
use crate::clients::Clients;
use crate::codecs::LineCodec;
use crate::config::{Config, Optional};
use crate::auth::Authentication;
use crate::throttle::ThrottledOutput;

use crate::connections::StreamRef;

impl<T: Evented + Read + Write> StreamRef<T> for TlsStream<Stream<T>> {
    fn stream(&self) -> &Stream<T> {
        self.get_ref()
    }

    fn stream_mut(&mut self) -> &mut Stream<T> {
        self.get_mut()
    }
}

type SelectedCodec = LineCodec;

pub fn serve(config: Config) -> Result<()> {
    let config = Arc::new(config);
    System::init().unwrap();
    let monitor_broadcast = Broadcast::unbounded(); 

    let tcp_listener_client  = Optional::new(config.use_tcp(), || {
        ReactiveTcpListener::bind(config.tcp_client_host()).unwrap() 
    });

    let tcp_listener_monitor = Optional::new(config.use_tcp(), || {
        ReactiveTcpListener::bind(config.tcp_monitor_host()).unwrap() 
    });

    let uds_listener_client  = Optional::new(config.use_uds(), || { 
        let _ = remove_file(config.uds_client_path());
        ReactiveUdsListener::bind(config.uds_client_path()).unwrap() 
    });

    let uds_listener_monitor = Optional::new(config.use_uds(), || { 
        let _ = remove_file(config.uds_monitor_path());
        ReactiveUdsListener::bind(config.uds_monitor_path()).unwrap() 
    });

    let mut tcp_client_queue  = ThrottledOutput::new(ReactiveQueue::unbounded())?;
    let mut tcp_monitor_queue = ThrottledOutput::new(ReactiveQueue::unbounded())?;
    let mut uds_client_queue  = ReactiveQueue::unbounded();
    let mut uds_monitor_queue = ReactiveQueue::unbounded();

    for _ in 0..7 { 
        let tcp_monitor_deque = tcp_monitor_queue.get_mut().deque();
        let tcp_clients_deque = tcp_client_queue.get_mut().deque();
        let uds_monitor_deque = uds_monitor_queue.deque();
        let uds_clients_deque = uds_client_queue.deque();

        let tcp_monitor_throttle = tcp_monitor_queue.sender();
        let tcp_clients_throttle = tcp_client_queue.sender();

        let monitor = monitor_broadcast.clone();
        let config = config.clone();

        thread::spawn(move || -> Result<()> {
            System::init().unwrap();
            // Tcp Clients
            let incoming_streams = ReactiveDeque::new(tcp_clients_deque)?;
            let tls_acceptor = ReactiveTlsAcceptor::new(&config.pfx_cert_path, &config.pfx_pass)?;
            let authentication = Authentication::new(config.clone(), Some(tcp_clients_throttle));
            let clients = Clients::<_, _, LineCodec>::new(monitor.subscriber())?;
            let tcp_clients = incoming_streams.chain(tls_acceptor.chain(authentication.chain(clients)));

            // Tcp Monitors
            let incoming_streams = ReactiveDeque::new(tcp_monitor_deque)?;
            let tls_acceptor = ReactiveTlsAcceptor::new(&config.pfx_cert_path, &config.pfx_pass)?;
            let authentication = Authentication::new(config.clone(), Some(tcp_monitor_throttle));
            let monitors = Monitors::<_, _, LineCodec>::new(monitor.clone());
            let tcp_monitors = incoming_streams.chain(tls_acceptor.chain(authentication.chain(monitors)));

            // Uds Clients
            let incoming_streams = ReactiveDeque::new(uds_clients_deque)?.map(|s| Stream::new(s).unwrap());
            let authentication = Authentication::new(config.clone(), None);
            let clients = Clients::<_, _, LineCodec>::new(monitor.subscriber())?;
            let uds_clients = incoming_streams.chain(authentication.chain(clients));

            // Uds Monitors
            let incoming_streams = ReactiveDeque::new(uds_monitor_deque)?.map(|s| Stream::new(s).unwrap());
            let authentication = Authentication::new(config.clone(), None);
            let monitors = Monitors::<_, _, LineCodec>::new(monitor);
            let uds_monitors = incoming_streams.chain(authentication.chain(monitors));

            System::start(
                tcp_monitors
                .and(tcp_clients)
                .and(uds_monitors)
                .and(uds_clients)
            )?;
            Ok(())
        });
    }

    System::start(
        tcp_listener_client.map(|(s, _)| s).chain(tcp_client_queue)
        .and(tcp_listener_monitor.map(|(s, _)| s).chain(tcp_monitor_queue))
        .and(uds_listener_client.map(|(s, _)| s).chain(uds_client_queue))
        .and(uds_listener_monitor.map(|(s, _)| s).chain(uds_monitor_queue))
    )?;
    
    Ok(())
}
