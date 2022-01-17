use futures::prelude::*;
use libp2p::swarm::{SwarmBuilder, SwarmEvent};
use libp2p::{identity, Multiaddr, PeerId};
use libp2p_perf::{build_transport, Perf, TransportSecurity};
use log::warn;
use std::time::{Duration, Instant};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "libp2p-perf client",
    about = "The iPerf equivalent for the libp2p ecosystem."
)]
struct Opt {
    #[structopt(long)]
    server_address: Multiaddr,

    #[structopt(long)]
    http_server_address: Option<Multiaddr>,

    #[structopt(long)]
    transport_security: Option<TransportSecurity>,
}

#[async_std::main]
async fn main() {
    env_logger::init();
    let opt = Opt::from_args();

    let key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(key.public());

    let transport = build_transport(
        false,
        key,
        opt.transport_security.unwrap_or(TransportSecurity::Noise),
    )
    .unwrap();
    let perf = Perf::default();
    let mut client = SwarmBuilder::new(transport, perf, local_peer_id)
        .executor(Box::new(|f| {
            async_std::task::spawn(f);
        }))
        .build();

    // Hack as Swarm::dial_addr does not accept Multiaddr with PeerId.
    let mut server_address = opt.server_address;
    if matches!(
        server_address.iter().last(),
        Some(libp2p::core::multiaddr::Protocol::P2p(_))
    ) {
        warn!("Ignoring provided PeerId.");
        server_address.pop().unwrap();
    }

    client.dial_addr(server_address).unwrap();

    loop {
        match client.next().await.expect("Infinite stream.") {
            SwarmEvent::Behaviour(e) => {
                println!("{}", e);

                // TODO: Fix hack
                //
                // Performance run timer has already been stopped. Wait for a second
                // to make sure the receiving side of the substream on the server is
                // closed before the whole connection is dropped.
                std::thread::sleep(std::time::Duration::from_secs(1));

                break;
            }
            SwarmEvent::ConnectionEstablished { .. } => {}
            e => panic!("{:?}", e),
        }
    }

    // http measurements
    if let Some(addr) = opt.http_server_address {
        http_get(addr).await;
    }
}

const SIZE: usize = 1024 * 1024;

async fn http_get(mut addr: Multiaddr) {
    use libp2p::core::multiaddr::Protocol;

    let port = if let Some(Protocol::Tcp(port)) = addr.pop() {
        port
    } else {
        panic!("invalid multiaddr, expected tcp, got {:?}", addr)
    };
    let host = if let Some(Protocol::Ip4(host)) = addr.pop() {
        host
    } else {
        panic!("invalid multiaddr, expected ipv4, got {:?}", addr)
    };

    println!("http-get: {}:{}/get", host, port);

    let mut bytes = 0;
    let start = Instant::now();

    while start.elapsed() < Duration::from_secs(10) {
        let res = surf::get(format!("http://{}:{}/get", host, port))
            .recv_bytes()
            .await
            .unwrap();
        assert_eq!(res.len(), SIZE);
        bytes += res.len();
    }

    println!(
        "HTTP: transferred {} MBytes in 10s",
        bytes as f32 / (1000. * 1000.)
    );
}
