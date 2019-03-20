use std::fs::remove_file;
use std::io::{Read, Write};
use std::sync::Arc;
use std::thread;

use sonr::errors::Result;
use sonr::net::tcp::ReactiveTcpListener;
use sonr::net::uds::ReactiveUdsListener;
use sonr::prelude::*;
use sonr::sync::broadcast::Broadcast;
use sonr::sync::queue::{ReactiveDeque, ReactiveQueue};
use sonr::Evented;
use sonr_tls::TlsAcceptor;

use crate::auth::{AuthMessage, Authentication};
use crate::clients::Clients;
use crate::codecs::LineCodec;
use crate::config::{Config, Optional};
use crate::connections::messages::Message;
use crate::monitors::Monitors;
use crate::throttle::ThrottledOutput;

fn tcp_listener(host: &str) -> ReactiveTcpListener {
    ReactiveTcpListener::bind(host).unwrap()
}

fn uds_listener(path: &str) -> ReactiveUdsListener {
    let _ = remove_file(path);
    ReactiveUdsListener::bind(path).unwrap()
}

fn tls<T: Evented + Read + Write>(config: &Config) -> TlsAcceptor<T> {
    TlsAcceptor::new(&config.pfx_cert_path, &config.pfx_pass).expect("fail")
}

pub fn serve(config: Config) -> Result<()> {
    let config = Arc::new(config);
    System::init()?;

    let broadcast = Broadcast::unbounded();

    // Tcp client
    let tcp_listener_client =
        Optional::new(config.use_tcp(), || tcp_listener(config.tcp_client_host()));
    let client_throttle = ThrottledOutput::new()?;
    let mut tcp_client_queue = ReactiveQueue::unbounded();

    // Uds client
    let uds_listener_client =
        Optional::new(config.use_uds(), || uds_listener(config.uds_client_path()));
    let mut uds_client_queue = ReactiveQueue::unbounded();

    // Tcp Monitor
    let tcp_listener_monitor =
        Optional::new(config.use_tcp(), || tcp_listener(config.tcp_monitor_host()));
    let monitor_throttle = ThrottledOutput::new()?;
    let mut tcp_monitor_queue = ReactiveQueue::unbounded();

    // Uds Monitor
    let uds_listener_monitor =
        Optional::new(config.use_uds(), || uds_listener(config.uds_monitor_path()));
    let mut uds_monitor_queue = ReactiveQueue::unbounded();

    for _ in 0..8 {
        let tcp_client_throttle = client_throttle.sender();
        let tcp_monitor_throttle = monitor_throttle.sender();
        let tcp_client_deque = tcp_client_queue.deque();
        let uds_client_deque = uds_client_queue.deque();
        let tcp_monitor_deque = tcp_monitor_queue.deque();
        let uds_monitor_deque = uds_monitor_queue.deque();
        let config = config.clone();
        let monitor = broadcast.clone();
        thread::spawn(move || -> Result<()> {
            System::init()?;

            // Tcp clients
            let tcp_client_deque = ReactiveDeque::new(tcp_client_deque)?.map(|s| Stream::new(s).unwrap());
            let cli_authentication = Authentication::<_, LineCodec<AuthMessage>, _>::new(
                config.clone(),
                Some(tcp_client_throttle),
            );
            let tcp_cli = Clients::<_, LineCodec<Message>>::new(monitor.subscriber())?;

            // Uds clients
            let uds_client_deque = ReactiveDeque::new(uds_client_deque)?.map(|s| Stream::new(s).unwrap());
            let uds_cli = Clients::<_, LineCodec<Message>>::new(monitor.subscriber())?;

            // Tcp monitors
            let tcp_monitor_deque = ReactiveDeque::new(tcp_monitor_deque)?.map(|s| Stream::new(s).unwrap());
            let mon_authentication = Authentication::<_, LineCodec<AuthMessage>, _>::new(
                config.clone(),
                Some(tcp_monitor_throttle),
            );
            let tcp_mon = Monitors::<_, LineCodec<Message>>::new(monitor.clone());

            // Uds monitors
            let uds_monitor_deque = ReactiveDeque::new(uds_monitor_deque)?.map(|s| Stream::new(s).unwrap());
            let uds_mon = Monitors::<_, LineCodec<Message>>::new(monitor);

            let tcp_client_run = tcp_client_deque.chain(tls(&config).chain(cli_authentication.chain(tcp_cli)));
            let uds_client_run = uds_client_deque.chain(uds_cli);
            let tcp_monitor_run = tcp_monitor_deque.chain(tls(&config).chain(mon_authentication.chain(tcp_mon)));
            let uds_monitor_run = uds_monitor_deque.chain(uds_mon);

            System::start(
                tcp_client_run
                .and(tcp_monitor_run)
                .and(uds_client_run)
                .and(uds_monitor_run)
            )?;
            Ok(())
        });
    }

    let tcp_client_run = tcp_listener_client
        .map(|(s, _)| s)
        .chain(client_throttle.chain(tcp_client_queue));

    let tcp_monitor_run = tcp_listener_monitor
        .map(|(s, _)| s)
        .chain(monitor_throttle.chain(tcp_monitor_queue));

    let uds_client_run = uds_listener_client
        .map(|(s, _)| s)
        .chain(uds_client_queue);

    let uds_monitor_run = uds_listener_monitor
        .map(|(s, _)| s)
        .chain(uds_monitor_queue);

    System::start(
        tcp_client_run
            .and(tcp_monitor_run)
            .and(uds_client_run)
            .and(uds_monitor_run),
    )?;
    Ok(())
}
