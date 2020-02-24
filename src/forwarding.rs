use std::thread;
use std::time::{SystemTime, Duration};
use std::sync::{mpsc, Arc};
use std::net::{TcpStream, UdpSocket, SocketAddr};
use std::io::prelude::*;
use std::io;
use crate::rules::{Protocol, Rule};

const TCP_INACTIVITY_TIMEOUT: u128 = 10000; //ms
const SET_TIMEOUT_FAIL: &str = "Failed to set socket read timeout";
const REMOTE_CONNECT_TIMEOUT: u64 = 5000; //ms

/// Data type received by a worker's rx
pub enum WorkerMsgInbound {
    // ask the worker to handle this tcp stream
    HandleTcp(TcpStream),

    // ask the worker to temporarily handle this udp socket
    HandleUdp(UdpSocket)
}

/// Data type sent by a worker's tx
pub enum WorkerMsgOutbound {
    // the worker is done
    Done,

    // the worker is done with this udp socket
    ReturnUdp(UdpSocket)
}

/// A worker responsible for handling packet forwarding
pub struct Worker {}

impl Worker {
    /// Create a new worker and return its JoinHandle
    pub fn new (
        rules: Arc<Vec<Rule>>,
        tx: mpsc::Sender<WorkerMsgOutbound>,
        rx: mpsc::Receiver<WorkerMsgInbound>
    ) -> thread::JoinHandle<()> {
        let thr = thread::spawn(move || {
            // main worker loop
            loop {
                let msg = rx.recv().unwrap();
                match msg {
                    WorkerMsgInbound::HandleTcp(stream) => {
                        handle_tcp(stream, &rules);
                    },
                    WorkerMsgInbound::HandleUdp(socket) => {
                        // when finished handling the udp socket it is returned
                        handle_udp(&socket);
                        tx.send(WorkerMsgOutbound::ReturnUdp(socket)).unwrap();
                    }
                }

                // signal that we are done
                tx.send(WorkerMsgOutbound::Done).unwrap();

                thread::sleep(Duration::from_millis(1));
            }
        });

        thr
    }
}

/// Called by a worker to handle a tcp stream
/// 
/// TCPStream is read from and forwarded to the destination specified by a matching rule,
/// until the read() function encounters an error of any kind.
fn handle_tcp (mut stream: TcpStream, rules: &Vec<Rule>) {
    // match with a rule
    let local_addr = match stream.local_addr() {
        Ok(addr) => addr,
        Err(_) => { return; }
    };

    let rule = match match_rule(Protocol::TCP, local_addr.port(), rules) {
        Some(rule) => rule,
        None => { return; }
    };

    // connect to rule's target address
    let mut outbound_stream = match TcpStream::connect_timeout(
        &SocketAddr::from(rule.target_address), 
        Duration::from_millis(REMOTE_CONNECT_TIMEOUT)
    ) {
        Ok(s) => s,
        Err(_) => { return; }
    };

    // set read timeouts
    let timeout = Some(Duration::from_millis(5));
    stream.set_read_timeout(timeout).expect(SET_TIMEOUT_FAIL);
    outbound_stream.set_read_timeout(timeout).expect(SET_TIMEOUT_FAIL);

    // forward data between streams until an error (not WouldBlock or TimedOut) is encountered
    // also breaks when the connection is unused for more than TCP_INACTIVITY_TIMEOUT

    let mut start_time = SystemTime::now();
    loop {
        let mut inbound_buf: Vec<u8> = Vec::with_capacity(1024);
        let mut outbound_buf: Vec<u8> = Vec::with_capacity(1024);

        // read from inbound stream

        if is_unexpected_err(stream.read_to_end(&mut inbound_buf)) {
            break;
        }

        // forward data
        if outbound_stream.write_all(&mut inbound_buf).is_err() { break; }
        if outbound_stream.flush().is_err() { break; }

        // read from outbound stream

        if is_unexpected_err(outbound_stream.read_to_end(&mut outbound_buf)) { break; }

        // forward data
        if stream.write_all(&mut outbound_buf).is_err() { break; }
        if stream.flush().is_err() { break; }

        let bytes_transfered = inbound_buf.len() + outbound_buf.len();
        if bytes_transfered > 0 {
            start_time = SystemTime::now();
        }

        // inactivity timeout
        if SystemTime::now().duration_since(start_time).unwrap().as_millis() 
            >= TCP_INACTIVITY_TIMEOUT {
                break;
        }
    }
}

/// Called by a worker to handle a udp socket
fn handle_udp (socket: &UdpSocket) {

}

/// Match a rule by port, returning a Rule or None
fn match_rule (protocol: Protocol, incomming_port: u16, rules: &Vec<Rule>) -> Option<&Rule> {
    for rule in rules {
        if rule.protocol == protocol && rule.listen_port == incomming_port {
            return Some(rule);
        }
    }

    None
}

/// Determines if a result is an error other than WouldBlock or TimedOut
/// 
/// Returns true only when res matches an unexpected error
fn is_unexpected_err<T> (res: Result<T, io::Error>) -> bool {
    match res {
        Err(ref e) 
            if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut
        => {
            false
        },
        Err(_) => true,
        _ => false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddrV4;

    #[test]
    fn test_match_rule () {
        let case = vec![
            Rule {
                protocol: Protocol::TCP,
                listen_port: 1234,
                target_address: SocketAddrV4::new(
                    "100.200.100.200".parse().unwrap(),
                    23456
                )
            },
            Rule {
                protocol: Protocol::UDP,
                listen_port: 2000,
                target_address: SocketAddrV4::new(
                    "100.200.100.200".parse().unwrap(),
                    22555
                )
            },
            Rule {
                protocol: Protocol::TCP,
                listen_port: 8080,
                target_address: SocketAddrV4::new(
                    "100.200.100.200".parse().unwrap(),
                    80
                )
            },
        ];

        // should match no rule
        let res = match_rule(Protocol::TCP, 23456, &case);
        assert_eq!(res, None);

        // should match a rule
        let res = match_rule(Protocol::UDP, 2000, &case);
        assert_eq!(res.unwrap().listen_port, 2000);
    }
}