use futures::prelude::*;
use libp2p::swarm::{SwarmBuilder, SwarmEvent};
use libp2p::{identity, Multiaddr, PeerId};
use libp2p_perf::{build_transport, Perf, TcpTransportSecurity};
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "libp2p-perf server",
    about = "The iPerf equivalent for the libp2p ecosystem."
)]
struct Opt {
    #[structopt(long)]
    tcp_listen_address: Option<Multiaddr>,
    #[structopt(long)]
    quic_listen_address: Option<Multiaddr>,

    #[structopt(long)]
    private_key_pkcs8: Option<PathBuf>,
}

#[async_std::main]
async fn main() {
    env_logger::init();
    let opt = Opt::from_args();

    let key = if let Some(path) = opt.private_key_pkcs8 {
        let mut bytes = std::fs::read(path).unwrap();
        identity::Keypair::rsa_from_pkcs8(&mut bytes).unwrap()
    } else {
        identity::Keypair::generate_ed25519()
    };
    let local_peer_id = PeerId::from(key.public());

    println!("Local peer id: {:?}", local_peer_id);
    let mut listen_addrs = vec![];

    if let Some(ref addr) = opt.quic_listen_address {
        listen_addrs.push(addr.clone());
    }
    if let Some(ref addr) = opt.tcp_listen_address {
        listen_addrs.push(addr.clone());
    }
    
    let transport = build_transport(
        key,
        TcpTransportSecurity::All,
        opt.quic_listen_address,
    )
    .unwrap();
    let perf = Perf::default();
    let mut server = SwarmBuilder::new(transport, perf, local_peer_id.clone())
        .executor(Box::new(|f| {
            async_std::task::spawn(f);
        }))
        .build();

    assert!(
        !listen_addrs.is_empty(),
        "Provide at least one listen address."
    );
    for addr in listen_addrs {
        println!("about to listen on {:?}", addr);
        server.listen_on(addr).unwrap();
    }

    loop {
        match server.next().await.unwrap() {
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("Listening on {:?}.", address);
            }
            e => {
                println!("{:?}", e);
            }
        }
    }
}
