use std::thread;
use std::sync::{mpsc, Arc};
use std::net::{TcpStream, UdpSocket};
use crate::rules::Rule;

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
            }
        });

        thr
    }
}

/// Called by a worker to handle a tcp stream
/// 
/// TCPStream is read from and forwarded to the destination specified by a matching rule,
/// until the read() function encounters an error of any kind.
fn handle_tcp (stream: TcpStream, rules: &Vec<Rule>) {
    // enforce non-blocking mode
    stream.set_nonblocking(false).expect("Failed to enforce non-blocking mode.");

    // match with a rule
    for rule in rules {
        println!("{:?}", rule);
    }
}

/// Called by a worker to handle a udp socket
fn handle_udp (socket: &UdpSocket) {

}