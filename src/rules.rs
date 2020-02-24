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

impl Rule {
    pub fn pretty_print(&self) -> String {
        let protostring = match self.protocol {
            Protocol::TCP => "TCP",
            Protocol::UDP => "UDP"
        };
        let target_string_ip = self.target_address.ip().to_string();
        let target_port = self.target_address.port();

        format!("[{}] {} -> {}:{}", protostring, self.listen_port, target_string_ip, target_port)
    }
}
