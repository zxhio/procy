use std::{
    error,
    io::Write,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};

use clap::{Arg, Command};
use log::{debug, error, info};
use tokio::{
    io::{self, AsyncWriteExt},
    net::{TcpListener, TcpSocket, TcpStream},
    time,
};

mod config;

include!(concat!(env!("OUT_DIR"), "/build_info.rs"));

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    let matches = Command::new("procy")
        .about("A simple proxy server")
        .version(COMMIT_VERSION_INFO)
        .arg(
            Arg::new("backend-addr")
                .long("backend-addr")
                .value_name("IP:PORT")
                .help("Destination address of the backend connection"),
        )
        .arg(
            Arg::new("source-addr")
                .long("source-addr")
                .value_name("IP:PORT")
                .help("Source address of the backend connection"),
        )
        .arg(
            Arg::new("source-port")
                .long("source-port")
                .value_name("PORT")
                .help("Source port of the backend connection"),
        )
        .arg(
            Arg::new("listen-addr")
                .long("listen-addr")
                .value_name("IP:PORT")
                .help("Address to listen on"),
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
                .value_names(&["listen,[source,]backend"])
                .num_args(1..)
                .help("Multi forward addresses combinations"),
        )
        .get_matches();

    let conf = config::Config::new(config::PROCY_CONFIG_PATH_ENV)?;
    set_logging(&conf);

    info!("procy {}", COMMIT_VERSION_INFO);

    let addr_tuples = get_forward_addr_tuples(&matches, &conf);
    let mut handles = Vec::new();
    for addr_tuple in addr_tuples {
        let listener = TcpListener::bind(addr_tuple.listen)
            .await
            .expect(format!("addr={}, tag={}", addr_tuple.listen, addr_tuple.tag).as_str());
        info!(
            "Listening on {} forward to {} tag={}",
            addr_tuple.listen, addr_tuple.backend, addr_tuple.tag
        );

        let handle = forward_loop(listener, addr_tuple);
        handles.push(tokio::spawn(handle));
    }
    for handle in handles {
        let _ = handle.await;
    }
    Ok(())
}

async fn forward_loop(listener: TcpListener, addr_tuple: ForwardAddrTuple) -> io::Result<()> {
    loop {
        let mut stream = listener.accept().await?;
        tokio::spawn(async move {
            let start_tm = time::Instant::now();
            let local_addr = stream.0.local_addr().unwrap_or(addr_tuple.listen);
            log_new_conn(stream.1, local_addr, addr_tuple.backend);
            match forward(&mut stream.0, addr_tuple.source, addr_tuple.backend).await {
                Ok((tx, rx)) => log_close_conn(stream.1, tx, rx, start_tm.elapsed()),
                Err(e) => log_close_conn_error(stream.1, local_addr, addr_tuple.backend, e),
            }
        });
    }
}

fn log_new_conn(remote_addr: SocketAddr, local_addr: SocketAddr, backend_addr: SocketAddr) {
    info!(
        "New conn from={} via={} to={}",
        remote_addr, local_addr, backend_addr
    );
}

fn log_close_conn(client_remote_addr: SocketAddr, tx: u64, rx: u64, dur: time::Duration) {
    debug!(
        "Closed conn from={} tx={} rx={} duration={:?} ",
        client_remote_addr, tx, rx, dur,
    );
}

fn log_close_conn_error(
    remote_addr: SocketAddr,
    local_addr: SocketAddr,
    backend_addr: SocketAddr,
    e: std::io::Error,
) {
    error!(
        "Fail to forward from={} via={} to={} error={}",
        remote_addr, local_addr, backend_addr, e
    )
}

#[derive(Debug)]
struct ForwardAddrTuple {
    listen: SocketAddr,
    backend: SocketAddr,
    source: Option<SocketAddr>,
    tag: String,
}

fn get_forward_addr_tuples(
    matches: &clap::ArgMatches,
    conf: &config::Config,
) -> Vec<ForwardAddrTuple> {
    let mut tuples = get_addr_tuples_by_addr_args(matches);
    tuples.extend(get_addr_tuples_by_forward_args(matches));
    tuples.extend(get_addr_tuples_by_config_file(conf));
    tuples
}

// from '--listen-addr', '--listen-port', '--source-addr', '--source-port' and '--backend-addr' command line args.
fn get_addr_tuples_by_addr_args(matches: &clap::ArgMatches) -> Vec<ForwardAddrTuple> {
    let backend_addr_opt = matches.get_one::<String>("backend-addr");
    let listen_addr_opt = matches.get_one::<String>("listen-addr");
    let listen_port_opt = matches.get_one::<String>("listen-port");
    let source_addr_opt = matches.get_one::<String>("source-addr");
    let source_port_opt = matches.get_one::<String>("source-port");

    let mut addr_tuples = Vec::new();

    if listen_addr_opt.is_none()
        && listen_port_opt.is_none()
        && source_addr_opt.is_none()
        && source_port_opt.is_none()
        && backend_addr_opt.is_none()
    {
        return addr_tuples;
    }

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

    let source_addr = if source_addr_opt.is_some() {
        Some(
            source_addr_opt
                .unwrap()
                .parse::<SocketAddr>()
                .expect(format!("invalid source-addr '{}'", source_addr_opt.unwrap()).as_str()),
        )
    } else if source_port_opt.is_some() {
        let port = source_port_opt
            .unwrap()
            .parse::<u16>()
            .expect(format!("invalid source-port '{}'", source_port_opt.unwrap()).as_str());

        if backend_addr.is_ipv6() {
            Some(SocketAddr::new(IpAddr::V6(Ipv6Addr::from_bits(0)), port))
        } else {
            Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::from_bits(0)), port))
        }
    } else {
        None
    };

    addr_tuples.push(ForwardAddrTuple {
        listen: listen_addr,
        source: source_addr,
        backend: backend_addr,
        tag: String::from("args-addr"),
    });
    addr_tuples
}

// from '--forward' command line arg.
fn get_addr_tuples_by_forward_args(matches: &clap::ArgMatches) -> Vec<ForwardAddrTuple> {
    let forward_opt = matches.get_many::<String>("forward");

    let mut addr_tuples = Vec::new();
    if forward_opt.is_none() {
        return addr_tuples;
    }

    for addr_tuple_str in forward_opt.unwrap() {
        let parts: Vec<&str> = addr_tuple_str.split(',').collect();
        if parts.len() != 2 && parts.len() != 3 {
            error!("Invalid forward rule format: {}", addr_tuple_str);
            std::process::exit(1);
        }

        let listen_addr = parts[0]
            .parse::<SocketAddr>()
            .expect(format!("invalid listen-addr '{}'", parts[0]).as_str());

        let source_addr = if parts.len() == 3 {
            Some(
                parts[1]
                    .parse::<SocketAddr>()
                    .expect(format!("invalid source-addr '{}'", parts[0]).as_str()),
            )
        } else {
            None
        };

        let backend_addr_str = if parts.len() == 2 { parts[1] } else { parts[2] };
        let backend_addr = backend_addr_str
            .parse::<SocketAddr>()
            .expect(format!("invalid backend-addr '{}'", parts[1]).as_str());

        addr_tuples.push(ForwardAddrTuple {
            listen: listen_addr,
            source: source_addr,
            backend: backend_addr,
            tag: String::from("args-forward"),
        });
    }

    addr_tuples
}

fn get_addr_tuples_by_config_file(conf: &config::Config) -> Vec<ForwardAddrTuple> {
    let mut addr_tuples = Vec::new();
    for tuple in &conf.forward_addresses {
        let listen_addr = match tuple.listen_addr {
            Some(addr) => addr,
            None => format!("[::]:{}", tuple.listen_port.unwrap())
                .parse::<SocketAddr>()
                .expect(format!("invalid listen-port '{}'", tuple.listen_port.unwrap()).as_str()),
        };

        let source_addr = match tuple.source_addr {
            Some(addr) => Some(addr),
            None => match tuple.source_port {
                Some(port) => {
                    if tuple.backend_addr.is_ipv6() {
                        Some(SocketAddr::new(IpAddr::V6(Ipv6Addr::from_bits(0)), port))
                    } else {
                        Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::from_bits(0)), port))
                    }
                }
                None => None,
            },
        };

        addr_tuples.push(ForwardAddrTuple {
            listen: listen_addr,
            source: source_addr,
            backend: tuple.backend_addr,
            tag: String::from("config"),
        });
    }

    addr_tuples
}

async fn connect_with_source_addr(
    source_addr: Option<SocketAddr>,
    backend_addr: SocketAddr,
) -> io::Result<TcpStream> {
    let socket = if backend_addr.is_ipv6() {
        TcpSocket::new_v6()?
    } else {
        TcpSocket::new_v4()?
    };

    if let Some(addr) = source_addr {
        socket.set_reuseaddr(true)?;
        socket.bind(addr)?;
    }
    Ok(socket.connect(backend_addr).await?)
}

async fn forward(
    client_stream: &mut TcpStream,
    source_addr: Option<SocketAddr>,
    backend_addr: SocketAddr,
) -> io::Result<(u64, u64)> {
    let mut backend_conn = connect_with_source_addr(source_addr, backend_addr).await?;
    let (tx, rx) = io::copy_bidirectional(client_stream, &mut backend_conn).await?;
    let _ = client_stream.shutdown().await;
    let _ = backend_conn.shutdown().await;
    Ok((tx, rx))
}

fn set_logging(conf: &config::Config) {
    let log_conf = match &conf.logging {
        Some(c) => c.clone(),
        None => config::LoggingConfig {
            path: None,
            level: None,
        },
    };

    let log_env = match log_conf.level {
        Some(level) => env_logger::Env::new().filter_or(env_logger::DEFAULT_FILTER_ENV, level),
        None => env_logger::Env::new().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    };

    let mut builder = env_logger::Builder::from_env(log_env);
    if let Some(path) = &log_conf.path {
        builder.target(env_logger::Target::Pipe(Box::new(LoggerWriter::new(path))));
    }

    builder
        .format_level(true)
        .format_timestamp_millis()
        .format_target(false)
        .init();
}

struct LoggerWriter {
    pub f: file_rotate::FileRotate<file_rotate::suffix::AppendTimestamp>,
}

impl LoggerWriter {
    fn new(path: &str) -> Self {
        let f = file_rotate::FileRotate::new(
            path,
            file_rotate::suffix::AppendTimestamp::default(
                file_rotate::suffix::FileLimit::MaxFiles(10),
            ),
            file_rotate::ContentLimit::Bytes(1024 * 1024 * 10),
            file_rotate::compression::Compression::None,
            #[cfg(unix)]
            None,
        );
        LoggerWriter { f }
    }
}

impl Write for LoggerWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.f.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
