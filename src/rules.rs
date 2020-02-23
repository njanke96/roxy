use std::net::SocketAddrV4;

/// Protocols
#[derive(PartialEq, Debug)]
pub enum Protocol {
    TCP,
    UDP,
}

/// An IPv4 forwarding rule
/// 
/// protocol: Protocol::TCP or Protocol::UDP
/// listen_port: the port the proxy server will listen on for this rule
/// target_address: the address and port the proxy server will forward traffic to
#[derive(PartialEq, Debug)]
pub struct Rule {
    pub protocol: Protocol,
    pub listen_port: u16,
    pub target_address: SocketAddrV4
}
