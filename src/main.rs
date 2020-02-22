mod cli;

use dns_lookup::lookup_host;
use std::net::IpAddr;

fn main() {
    let hostname = "www.reddit.com";
    let ips: Vec<IpAddr> = lookup_host(hostname).unwrap();
    for ip in ips {
        println!("found ip: {:?}", ip);
    }
}
