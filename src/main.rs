mod tcp_server;
mod thread_pool;
mod ws_server;

use std::net::{IpAddr, Ipv4Addr};

use tracing::{info, Level};
use ws_server::WsServer;

use crate::tcp_server::TcpServer;

fn main() {
    use tracing_subscriber::FmtSubscriber;

    // Trace initialization
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("Could not set default trace subscriver!");

    let mut ip: String = "127.0.0.1".into();
    let mask: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);

    for iface in pnet::datalink::interfaces() {
        info!("Found address: {:?}", iface.ips);

        let addr = iface.ips[0];
        if !addr.contains(IpAddr::V4(mask)) {
            ip = addr.ip().to_string();
        }
    }

    let ip_clone = ip.clone();

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(TcpServer::run(format!("{}{}", ip_clone, ":8080")));
    });

    WsServer::new(format!("{}{}", ip, ":7878"));
}
