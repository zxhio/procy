# procy

A simple reverse proxy written in rust.

## Usage

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

## TODO
- Support for proxies with multiple addresses.
- Support specify backend connection local address.
