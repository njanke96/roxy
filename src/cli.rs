use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use dns_lookup;
use crate::rules::{Protocol, Rule};


/// Parse command line arguments
/// If --help is a token, will always display help and return None
/// 
/// Rule syntax: --<tcp|udp> <incomming port>:<target host/ip>:<target port>
/// Rule example: --tcp 8080:www.reddit.com:80
pub fn parse_args(args: Vec<String>, bind_addr: &mut Ipv4Addr) -> Option<Vec<Rule>> {
    let invalid_rule_syntax = &"Invalid rule syntax. See --help.";

    let mut parsed_rules: Vec<Rule> = Vec::with_capacity(8);
    let mut expect_rule: &Option<Protocol> = &None;
    let mut expect_bind_addr = false;
    for token in args.into_iter() {
        if token == "--help" {
            print_help();
            return None;
        }

        if token == "--tcp" {
            expect_rule = &Some(Protocol::TCP);
            continue;
        }

        if token == "--udp" {
            expect_rule = &Some(Protocol::UDP);
            continue;
        }

        if token == "--bind" {
            expect_bind_addr = true;
            continue;
        }

        if expect_bind_addr {
            // set the bind address
            let new_bind_addr: Ipv4Addr = match token.parse() {
                Ok(addr) => addr,
                Err(_) => {
                    printerr("Invalid address specified after --bind.");
                    return None;
                }
            };

            *bind_addr = new_bind_addr;

            expect_bind_addr = false;
            continue;
        }

        // TODO: Make some of this dry if possible?
        match expect_rule {
            Some(protocol) => {
                // parse rule
                let segs: Vec<&str> = token.split(":").collect();
                
                // listen port
                let listen_port = match segs.get(0) {
                    Some(port) => port,
                    None => {
                        printerr(invalid_rule_syntax);
                        return None;
                    }
                };

                let listen_port: u16 = match listen_port.parse() {
                    Ok(port) => port,
                    Err(_) => {
                        printerr(invalid_rule_syntax);
                        return None;
                    }
                };

                // host
                let host = match segs.get(1) {
                    Some(host) => host,
                    None => {
                        printerr(invalid_rule_syntax);
                        return None;
                    }
                };

                // parse host to Ipv4Addr
                let host: Ipv4Addr = match host.parse() {
                    Ok(valid_host) => valid_host,
                    Err(_) => {
                        // Try resolving hostname
                        let resolved_host: Option<Ipv4Addr> = match dns_lookup::lookup_host(host) {
                            Ok(addrs) => {
                                let mut addr_from_host: Option<Ipv4Addr> = None;
                                for addr in addrs.into_iter() {
                                    let v4addr: Option<Ipv4Addr> = match addr {
                                        IpAddr::V4(v4) => Some(v4),
                                        IpAddr::V6(_) => None
                                    };

                                    // take the first v4 address
                                    if v4addr != None {
                                        addr_from_host = v4addr;
                                        break;
                                    }
                                }

                                addr_from_host
                            },
                            Err(_) => None
                        };

                        if resolved_host == None {
                            printerr(
                                &format!(
                                    "Could not resolve hostname \"{}\" to an IPV4 Address. \
                                    Rule will not be added.", host
                                ).to_string()
                            );
                            expect_rule = &None;
                            continue;
                            
                        }

                        resolved_host.unwrap()
                    },
                };


                // target port
                let target_port = match segs.get(2) {
                    Some(port) => port,
                    None => {
                        printerr(invalid_rule_syntax);
                        return None;
                    }
                };

                let target_port: u16 = match target_port.parse() {
                    Ok(port) => port,
                    Err(_) => {
                        printerr(invalid_rule_syntax);
                        return None;
                    }
                };

                // append the rule
                parsed_rules.push(Rule {
                    protocol: match protocol {
                        Protocol::TCP => Protocol::TCP,
                        Protocol::UDP => Protocol::UDP
                    },
                    listen_port,
                    target_address: SocketAddrV4::new(host, target_port)
                });

                expect_rule = &None;
            },
            None => ()
        }
    }

    // return None instead of empty rule vec
    if parsed_rules.is_empty() {
        None
    } else {
        Some(parsed_rules)
    }
}

/// Print to stdout
pub fn printmsg(message: &str) {
    println!("roxy: {}", message);
}

/// Print to stderr
pub fn printerr(message: &str) {
    println!("roxy: {}", message);
}

/// Print help message
fn print_help() {
    let help = "ROXY Help:\n\n\
        Command-line usage: roxy <rules> [--bind <address>]\n\
        Rule Syntax: --<tcp|udp> <incomming port>:<target host/ip>:<target port>\n\n\
        Example: roxy --tcp 8080:localhost:80\n\
        Example: roxy --udp 3443:192.0.0.1:2550 --bind 100.101.102.103\n";

    print!("{}", help);
}

/* Unit tests */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_args() {
        let mut bind_addr = Ipv4Addr::new(127, 0, 0, 1);

        // Given help it should return None
        let res = parse_args(vec!["--help".to_owned()], &mut bind_addr);
        assert_eq!(res, None);

        // Given help with a rule it should still return None
        let res = parse_args(vec![
            "--help".to_owned(),
            "--tcp".to_owned(),
            "1234:12.22.32.43:8080".to_owned()
        ], &mut bind_addr);

        assert_eq!(res, None);

        // Parse a tcp rule, a tcp rule from localhost, and a udp rule
        let res = parse_args(vec![
            "--tcp".to_owned(),
            "8080:192.0.0.1:80".to_owned(),
            "--tcp".to_owned(),
            "8000:localhost:80".to_owned(),
            "--udp".to_owned(),
            "1234:212.53.12.49:2550".to_owned()
        ], &mut bind_addr);

        let spec: Vec<Rule> = vec![
            Rule {
                protocol: Protocol::TCP,
                listen_port: 8080,
                target_address: SocketAddrV4::new(Ipv4Addr::new(192, 0, 0, 1), 80),
            },
            Rule {
                protocol: Protocol::TCP,
                listen_port: 8000,
                target_address: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 80),
            },
            Rule {
                protocol: Protocol::UDP,
                listen_port: 1234,
                target_address: SocketAddrV4::new(Ipv4Addr::new(212, 53, 12, 49), 2550),
            }
        ];

        assert_eq!(res.unwrap(), spec);

        // Change the bind address
        assert_eq!("127.0.0.1".parse(), Ok(bind_addr));
        let new_addr = "128.129.130.131";
        parse_args(vec!["--bind".to_owned(), new_addr.to_owned()], &mut bind_addr);
        assert_eq!(new_addr.parse(), Ok(bind_addr));
    }
}
