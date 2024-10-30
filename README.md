# procy

A simple reverse proxy written in rust.

## Usage

### Command line

See `procy --help`

Here is an example of how to forward data from port 10022 to 127.0.0.1:22 .
```shell
$ procy --listen-port 10022 --backend-addr 127.0.0.1:22
```

You can also specify ip.
```shell
$ procy --listen-addr 192.168.10.2:10022 --backend-addr 127.0.0.1:22
```

IPv6 also supports.
```shell
$ procy --listen-addr [::]:10022 --backend-addr 127.0.0.1:22
```

Specify multi address pair for fowarding.
```shell
$ procy --forward 192.168.10.2:10022,127.0.0.1:22
$ procy --forward 192.168.10.2:10022,127.0.0.1:22 192.168.10.2:10023,127.0.0.1:22
```

### Daemon service

In the case of a daemon, the configuration file can be specified through environment variable **PROCY_CONFIG_PATH** to avoid the need to execute *daemon-reload* every time the address is modified.

This is an example of a **procy** configuration where two forwarding address pairs have been configured. 
```toml
[log]
log_level="debug"
log_path="/path/to/log"

[[forward_addr_pairs]]
listen_addr = "192.168.32.251:10022"
backend_addr = "127.0.0.1:22"

[[forward_addr_pairs]]
listen_addr = "192.168.32.251:10023"
backend_addr = "127.0.0.1:22"
```

#### systemd

Here is an systemd service.
```shell
[Unit]
Description=Simple Proxy Server

[Service]
Type=simple
WorkingDirectory=
Environment=PROCY_CONFIG_PATH=/path/to/procy_config
ExecStart=/path/to/procy
Restart=always
RestartSec=3s
KillMode=process

[Install]
WantedBy=multi-user.target
```

Write it into **/usr/lib/systemd/system/procy.service** and run command.
```shell
$ systemctl daemon-reload
$ systemctl enable procy
$ systemctl start procy
```

## TODO
- Support specify backend connection local address.
