# Remonitor

Remote monitor server.

Example config

Copy `config.example.toml` and update with suitable values.

```
uds_monitor_path = "/tmp/remonitor-monitor.sock"
uds_client_path = "/tmp/remonitor-client.sock"
tcp_client_host = "0.0.0.0:5555"
tcp_monitor_host = "0.0.0.0:5556"
thread_count = 8
pfx_cert_path = "/path/to/pfx"
pfx_pass = "<password>"

[auth]
"client1" = "password1"
"monitor1" = "password2"
```

To disable either tcp or uds remove the path / host information from the config.

# Sending and receiving messages:

Messages are json encoded and separated by a newline character `\n`.

## Authenticating:

Send two messages. The first one is the username and the second one is the
password.

`{"payload": "username"}` followed by `{"payload": "password"}`

Upon successful authentication an "OK" message is sent in response. 

```{"payload": "OK", "message_type": "status"}```

A client can then start receiving updates.
A monitor is then able to start sending updates.
