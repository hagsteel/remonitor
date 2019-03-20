use std::io::ErrorKind;

use sonr::errors::Error;
use docopt::Docopt;
use serde::Deserialize;

use remonitor::config::Config;
use remonitor::server;

#[derive(Debug, Deserialize)]
struct Opts {
    arg_config: Option<String>,
}

const USAGE: &'static str = "
Remonitor

Usage:
    remonitor
    remonitor (-c | --config) <config>

Options:
    -c --config  config file path
";

fn print_err(e: Error, file_path: &str) {
    match e {
        Error::Io(io_err) => {
            match io_err.kind() {
                ErrorKind::NotFound => eprintln!("The file \"{}\" can not be found", file_path),
                _ => {}
            }
        }
        _ => {}
    }
}

fn main() {
    let options: Opts = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    let config_file_path = options.arg_config.unwrap_or("config.toml".into());

    let config = Config::from_file(&config_file_path).unwrap_or_else(|e| {
        print_err(e, &config_file_path); 
        std::process::exit(1)
    });

    let mut msg = r"
Remonitor server
----------------
"
    .to_owned();

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
