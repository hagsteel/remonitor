# Remonitor

Remote monitor server.

Example config

```
uds_monitor_path = "/tmp/remonitor-monitor.sock"
uds_client_path = "/tmp/remonitor-client.sock"
tcp_client_host = "0.0.0.0:5555"
tcp_monitor_host = "0.0.0.0:5556"
thread_count = 7
pfx_cert_path = "/path/to/pfx"
pfx_pass = ""
enable_log = true

[auth]
"client1" = "password1"
"monitor1" = "password2"
```
