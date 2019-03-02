use remonitor::server;
use remonitor::config::Config;

fn main() {
    let config = Config::from_file("config.toml").unwrap();
    let mut msg = r"
Remonitor server
----------------
".to_owned();

    msg.push_str(&format!("running with {} threads\n", config.thread_count));

    if config.use_tcp() {
        msg.push_str("\nTcp:\n");
        msg.push_str(&format!("client  host: {}\n", config.tcp_client_host()));
        msg.push_str(&format!("monitor host: {}\n", config.tcp_monitor_host()));
    }

    if config.use_uds() {
        msg.push_str("\nUnix domain socket:\n");
        msg.push_str(&format!("client  host: {}\n", config.uds_client_path()));
        msg.push_str(&format!("monitor host: {}\n", config.uds_monitor_path()));
    }

    println!("{}", msg);
    server::serve(config);
}
