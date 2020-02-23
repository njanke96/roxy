use std::thread;
use std::sync::mpsc;
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
    // the worker is busy
    Busy,

    // the worker is free
    Free,

    // the worker is done with this udp socket
    ReturnUdp(UdpSocket)
}

/// A worker responsible for handling packet forwarding
pub struct Worker<'a> {
    rules: &'a Vec<Rule>,
    thread_handle: thread::JoinHandle<()>
}

impl<'a> Worker<'a> {
    pub fn new (
        rules: &Vec<Rule>,
        tx: mpsc::Sender<WorkerMsgOutbound>,
        rx: mpsc::Receiver<WorkerMsgInbound>
    ) -> Worker {
        let thr = thread::spawn(move || {
            loop {
                let msg = rx.recv().unwrap();
                tx.send(WorkerMsgOutbound::Busy).unwrap();
                match msg {
                    WorkerMsgInbound::HandleTcp(stream) => handle_tcp(stream),
                    WorkerMsgInbound::HandleUdp(socket) => {
                        // when finished handling the udp socket it is returned
                        handle_udp(&socket);
                        tx.send(WorkerMsgOutbound::ReturnUdp(socket)).unwrap();
                    }
                }
                tx.send(WorkerMsgOutbound::Free).unwrap();
            }
        });

        Worker {
            rules,
            thread_handle: thr
        }
    }
}

/// Called by a worker to handle a tcp stream
fn handle_tcp (stream: TcpStream) {

}

/// Called by a worker to handle a udp socket
fn handle_udp (socket: &UdpSocket) {

}