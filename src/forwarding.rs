use std::thread;
use std::time::{SystemTime, Duration};
use std::sync::{mpsc, Arc};
use std::net::{TcpStream, UdpSocket, SocketAddr, SocketAddrV4};
use std::io::prelude::*;
use std::io;
use crate::rules::{Protocol, Rule};
use crate::cli;

const TCP_INACTIVITY_TIMEOUT: u128 = 100; //ms
const SET_TIMEOUT_FAIL: &str = "Failed to set socket read timeout";
const REMOTE_CONNECT_TIMEOUT: u64 = 5000; //ms

/// Data type received by a worker's rx
pub enum WorkerMsgInbound {
    // ask the worker to handle this tcp stream
    HandleTcp(TcpStream),

    // ask the worker to temporarily handle this udp socket
    HandleUdp(StatedUdpSocket)
}

/// Data type sent by a worker's tx
pub enum WorkerMsgOutbound {
    // the worker is done
    Done,

    // the worker is done with this Tcp Stream
    ReturnTcp(TcpStream),

    // the worker is done with this udp socket
    ReturnUdp(StatedUdpSocket)
}

/// A container for a UdpSocket, which also contains the address of the
/// most recent incomming connection (incomming to the proxy server)
pub struct StatedUdpSocket {
    pub socket: UdpSocket,
    pub last_client: Option<SocketAddrV4>
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
                        // if a tcp stream is returned (and not None), return it to the main thr
                        match handle_tcp(stream, &rules) {
                            Some(s) => tx.send(WorkerMsgOutbound::ReturnTcp(s)).unwrap(),
                            None => ()
                        }
                    },
                    WorkerMsgInbound::HandleUdp(stated_socket) => {
                        // when finished handling the udp stated_socket it is returned to main thr
                        let stated_socket = handle_udp(stated_socket, &rules);
                        tx.send(WorkerMsgOutbound::ReturnUdp(stated_socket)).unwrap();
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
/// 
/// Returns the TcpStream if it is to be returned to the main thread (it is idle)
/// Returns None if an error was encountered and the stream was discarded.
fn handle_tcp (mut stream: TcpStream, rules: &Vec<Rule>) -> Option<TcpStream> {
    // match with a rule
    let local_addr = match stream.local_addr() {
        Ok(addr) => addr,
        Err(_) => { return None; }
    };

    let rule = match match_rule(Protocol::TCP, local_addr.port(), rules) {
        Some(rule) => rule,
        None => { return None; }
    };

    // connect to rule's target address
    let mut outbound_stream = match TcpStream::connect_timeout(
        &SocketAddr::from(rule.target_address), 
        Duration::from_millis(REMOTE_CONNECT_TIMEOUT)
    ) {
        Ok(s) => s,
        Err(_) => { return None; }
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
            return None;
        }

        // forward data
        if outbound_stream.write_all(&mut inbound_buf).is_err() { return None; }
        if outbound_stream.flush().is_err() { return None; }

        // read from outbound stream

        if is_unexpected_err(outbound_stream.read_to_end(&mut outbound_buf)) { return None; }

        // forward data
        if stream.write_all(&mut outbound_buf).is_err() { return None; }
        if stream.flush().is_err() { return None; }

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

    Some(stream)
}

/// Called by a worker to handle a udp socket
/// Always returns the socket after we are done
fn handle_udp (mut stated_socket: StatedUdpSocket, rules: &Vec<Rule>) -> StatedUdpSocket {
    // match with a rule
    let local_addr = match stated_socket.socket.local_addr() {
        Ok(addr) => addr,
        Err(_) => { return stated_socket; }
    };

    let rule = match match_rule(Protocol::UDP, local_addr.port(), rules) {
        Some(rule) => rule,
        None => { return stated_socket; }
    };

    // read all there is to read
    let mut buf = [0; 32768];
    let mut last_len = 1;
    let mut from_addr: Option<SocketAddr> = None;
    while last_len > 0 {
        let res = stated_socket.socket.recv_from(&mut buf);

        // wouldblock, empty, other
        if res.is_err() { break; }

        let (len, from) = res.unwrap();
        last_len = len;
        from_addr = Some(from);
    }

    if buf.len() < 1 || from_addr == None {
        // no data
        return stated_socket;
    }

    // get the socket address of the sender
    let from = match from_addr.unwrap() {
        SocketAddr::V4(addr) => addr,
        _ => { return stated_socket; }
    };

    let send_res: Result<usize, std::io::Error>;

    if from == rule.target_address {
        // datagram is coming from the target, attempt to send it to the most recent client
        if stated_socket.last_client != None {
            let last_client = stated_socket.last_client.unwrap();
            send_res = stated_socket.socket.send_to(&buf, last_client);
        } else {
            return stated_socket;
        }
    } else {
        // datagram is not coming from the target, so send it to the target
        send_res = stated_socket.socket.send_to(&buf, rule.target_address);

        // record this address as the most recent client
        stated_socket.last_client = Some(from);
    }

    // check result
    match send_res {
        Ok(_) => (),
        Err(e) => cli::printerr(&format!("Supressed an error while sending UDP packet: {}", e))
    }

    stated_socket
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