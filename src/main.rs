use std::{fs::File, io::Read, net::SocketAddr};

use clap::{Arg, Command};
use tokio::{
    io::{self, AsyncWriteExt},
    net::{TcpListener, TcpSocket, TcpStream},
    time,
};

mod config;

include!(concat!(env!("OUT_DIR"), "/build_info.rs"));

#[tokio::main]
async fn main() -> io::Result<()> {
    let matches = Command::new("procy")
        .about("A simple proxy server")
        .version(COMMIT_VERSION_INFO)
        .arg(
            Arg::new("backend-addr")
                .long("backend-addr")
                .value_name("ADDR")
                .help("Remote address (format 'ip:port') to forward to"),
        )
        .arg(
            Arg::new("listen-addr")
                .long("listen-addr")
                .value_name("ADDR")
                .help("Address (format 'ip:port') to listen on"),
        )
        .arg(
            Arg::new("listen-port")
                .long("listen-port")
                .value_name("PORT")
                .help("Port to listen on"),
        )
        .arg(
            Arg::new("forward")
                .long("forward")
                .value_names(["listen-addr,backend-addr"])
                .num_args(0..)
                .help("Multiple listen-addr and backend-addr combinations"),
        )
        .get_matches();

    let addr_pairs = get_addr_pairs(&matches);
    let mut handles = Vec::new();
    for pair in addr_pairs {
        let listener = TcpListener::bind(pair.listen)
            .await
            .expect(pair.listen.to_string().as_str());
        println!(
            "Listening on {} forward to {} tag={}",
            pair.listen, pair.backend, pair.tag
        );

        let handle = forward_loop(listener, pair);
        handles.push(tokio::spawn(handle));
    }
    for handle in handles {
        let _ = handle.await;
    }
    Ok(())
}

async fn forward_loop(listener: TcpListener, addr_pair: ForwardAddrPair) -> io::Result<()> {
    loop {
        let mut stream = listener.accept().await?;
        tokio::spawn(async move {
            let start_tm = time::Instant::now();
            let local_addr = stream.0.local_addr().unwrap_or(addr_pair.listen);
            log_new_conn(stream.1, local_addr, addr_pair.backend);
            match forward(&mut stream.0, addr_pair.backend).await {
                Ok((tx, rx)) => log_close_conn(stream.1, tx, rx, start_tm.elapsed()),
                Err(e) => log_close_conn_error(stream.1, local_addr, addr_pair.backend, e),
            }
        });
    }
}

fn log_new_conn(
    client_remote_addr: SocketAddr,
    client_local_addr: SocketAddr,
    backend_addr: SocketAddr,
) {
    println!(
        "New conn from={} via={} to={}",
        client_remote_addr, client_local_addr, backend_addr
    );
}

fn log_close_conn(
    client_remote_addr: SocketAddr,
    tx_bytes: u64,
    rx_bytes: u64,
    dur: time::Duration,
) {
    println!(
        "Closed conn from={} tx={} rx={} duration={:?} ",
        client_remote_addr, tx_bytes, rx_bytes, dur,
    );
}

fn log_close_conn_error(
    client_remote_addr: SocketAddr,
    client_local_addr: SocketAddr,
    backend_addr: SocketAddr,
    e: std::io::Error,
) {
    eprintln!(
        "Fail to forward from={} via={} to={} error={}",
        client_remote_addr, client_local_addr, backend_addr, e
    )
}

#[derive(Debug)]
struct ForwardAddrPair {
    listen: SocketAddr,
    backend: SocketAddr,
    tag: String,
}

fn get_addr_pairs(matches: &clap::ArgMatches) -> Vec<ForwardAddrPair> {
    let backend_addr_opt = matches.get_one::<String>("backend-addr");
    let listen_addr_opt = matches.get_one::<String>("listen-addr");
    let listen_port_opt = matches.get_one::<String>("listen-port");
    let forward_opt = matches.get_many::<String>("forward");

    let mut addr_pairs = Vec::new();

    // from '--forward' command line arg.
    if forward_opt.is_some() {
        for addr_pair_str in forward_opt.unwrap() {
            let parts: Vec<&str> = addr_pair_str.split(',').collect();
            if parts.len() != 2 {
                eprintln!("Invalid forward rule format: {}", addr_pair_str);
                std::process::exit(1);
            }

            addr_pairs.push(ForwardAddrPair {
                listen: parts[0]
                    .parse::<SocketAddr>()
                    .expect(format!("invalid listen-addr '{}'", parts[0]).as_str()),
                backend: parts[1]
                    .parse::<SocketAddr>()
                    .expect(format!("invalid backend-addr '{}'", parts[1]).as_str()),
                tag: String::from("CMD"),
            });
        }
    }

    // from '--listen-addr', '--listen-port' and '--backend-addr' command line args.
    if listen_addr_opt.is_some() || listen_port_opt.is_some() || backend_addr_opt.is_some() {
        let listen_addr = if listen_addr_opt.is_some() {
            listen_addr_opt
                .unwrap()
                .parse::<SocketAddr>()
                .expect(format!("invalid listen-addr '{}'", listen_addr_opt.unwrap()).as_str())
        } else {
            format!("[::]:{}", listen_port_opt.unwrap())
                .parse::<SocketAddr>()
                .expect(format!("invalid listen-port '{}'", listen_port_opt.unwrap()).as_str())
        };

        let backend_addr = backend_addr_opt
            .unwrap()
            .parse::<SocketAddr>()
            .expect(format!("invalid backend-addr '{}'", backend_addr_opt.unwrap()).as_str());

        addr_pairs.push(ForwardAddrPair {
            listen: listen_addr,
            backend: backend_addr,
            tag: String::from("CMD"),
        });
    }

    // from ENV VAR 'PROCY_FORWARD_ADDR_PATH'
    if let Ok(addr_conf_path) = std::env::var(config::PROCY_CONFIG_PATH_ENV) {
        let mut file = File::open(&addr_conf_path).expect(&addr_conf_path);
        let mut contents = String::new();
        file.read_to_string(&mut contents).expect(&addr_conf_path);
        let conf: config::Config = toml::from_str(&contents).expect(&addr_conf_path);
        for pair in conf.forward_addr_pairs {
            addr_pairs.push(ForwardAddrPair {
                listen: pair.listen_addr.parse::<SocketAddr>().expect(
                    format!("invalid env conf listen-addr '{}'", pair.listen_addr).as_str(),
                ),
                backend: pair.backend_addr.parse::<SocketAddr>().expect(
                    format!("invalid env conf backend-addr '{}'", pair.listen_addr).as_str(),
                ),
                tag: String::from("ENV"),
            });
        }
    }

    addr_pairs
}

async fn connect_with_local_addr(
    local_addr: Option<SocketAddr>,
    backend_addr: SocketAddr,
) -> io::Result<TcpStream> {
    let socket = if backend_addr.is_ipv6() {
        TcpSocket::new_v6()?
    } else {
        TcpSocket::new_v4()?
    };

    if let Some(addr) = local_addr {
        socket.bind(addr)?;
    }
    Ok(socket.connect(backend_addr).await?)
}

async fn forward(
    client_stream: &mut TcpStream,
    backend_addr: SocketAddr,
) -> io::Result<(u64, u64)> {
    let mut backend_conn = connect_with_local_addr(None, backend_addr).await?;
    let (tx, rx) = io::copy_bidirectional(client_stream, &mut backend_conn).await?;
    let _ = client_stream.shutdown().await;
    let _ = backend_conn.shutdown().await;
    Ok((tx, rx))
}
