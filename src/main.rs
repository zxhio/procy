use clap::{Arg, Command};
use std::net::SocketAddr;
use tokio::{
    io::{self, AsyncWriteExt},
    net::{TcpListener, TcpSocket, TcpStream},
    time,
};

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
                .help("Remote address to forward to (e.g., 127.0.0.1:8080)")
                .required(true),
        )
        .arg(
            Arg::new("listen-addr")
                .long("listen-addr")
                .value_name("ADDR")
                .help("Address to listen on (e.g., 192.168.110.200:8080)"),
        )
        .arg(
            Arg::new("listen-port")
                .long("listen-port")
                .value_name("PORT")
                .help("Port to listen on (e.g., 8080)"),
        )
        .get_matches();

    let backend_addr_opt = matches.get_one::<String>("backend-addr");
    let listen_addr_opt = matches.get_one::<String>("listen-addr");
    let listen_port_opt = matches.get_one::<String>("listen-port");

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

    let listener = TcpListener::bind(listen_addr).await?;
    println!("Listening on {}", listen_addr);

    loop {
        let mut client_stream = listener.accept().await?;
        tokio::spawn(async move {
            let start_tm = time::Instant::now();
            let client_local_addr = client_stream.0.local_addr().unwrap_or(listen_addr);
            println!(
                "New conn from={} via={} to={}",
                client_stream.1, client_local_addr, backend_addr
            );

            match copy_stream(&mut client_stream.0, backend_addr).await {
                Ok((tx, rx)) => {
                    println!(
                        "Closed conn from={} send_bytes={} recv_bytes={} duration={:?} ",
                        client_stream.1,
                        tx,
                        rx,
                        start_tm.elapsed(),
                    );
                }
                Err(e) => {
                    eprintln!(
                        "Fail to forward from={} via={} to={} error={}",
                        client_stream.1, client_local_addr, backend_addr, e
                    )
                }
            }
        });
    }
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

async fn copy_stream(
    client_stream: &mut TcpStream,
    backend_addr: SocketAddr,
) -> io::Result<(u64, u64)> {
    let mut backend_conn = connect_with_local_addr(None, backend_addr).await?;
    let (tx, rx) = io::copy_bidirectional(client_stream, &mut backend_conn).await?;
    let _ = client_stream.shutdown().await;
    let _ = backend_conn.shutdown().await;
    Ok((tx, rx))
}
