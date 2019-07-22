// Currently excluded from module tree. Rand adds a ton of dependencies and slows build time. :(

use std::net::{Ipv4Addr, SocketAddrV4, SocketAddr};

use rand::{self, Rng};

pub fn random_ipv4_addr(port: u16) -> SocketAddr {
    let random_bytes = rand::thread_rng().gen::<[u8; 4]>();
    let ip = Ipv4Addr::new(
        random_bytes[0],
        random_bytes[1],
        random_bytes[2],
        random_bytes[3]);

    SocketAddrV4::new(ip, port)
        .into()
}