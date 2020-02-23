mod cli;

use std::net::Ipv4Addr;
use std::env;

fn main() {
    // bind address is initially localhost
    let mut bind_address = Ipv4Addr::LOCALHOST;
    let rules = cli::parse_args(env::args().collect(), &mut bind_address);
    for rule in rules.into_iter() {
        println!("{:?}", rule);
    }

    println!("\nbind_address: {:?}", bind_address);
}
