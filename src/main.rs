mod cli;
mod rules;
mod forwarding;

use std::net;
use std::env;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;
use forwarding::{Worker, WorkerMsgInbound, WorkerMsgOutbound};
use rules::Protocol;

const NON_BLOCKING_FAIL: &str = "Failed to enforce non-blocking mode";

struct ActiveWorker {
    // the sender for sending messages to the worker
    sender: mpsc::Sender<WorkerMsgInbound>,

    // the receiver for receiving messages from the worker
    receiver: mpsc::Receiver<WorkerMsgOutbound>,

    is_idle: bool
}

fn main() {
    // bind address is initially localhost
    let mut bind_address = net::Ipv4Addr::LOCALHOST;

    // max workers is initially 8
    let mut max_workers: usize = 8;

    // parse cmd args
    let rules = cli::parse_args(env::args().collect(), &mut bind_address, &mut max_workers);
    if rules == None {
        cli::printmsg("No rules specified.");
        return;
    }

    // arc pointing to our rules
    let rules: Arc<Vec<rules::Rule>> = Arc::new(rules.unwrap());

    // vectors holding non-blocking TcpListeners, and UdpSockets bound according to rules
    let mut tcp_listeners: Vec<net::TcpListener> = Vec::with_capacity(rules.len());
    let mut udp_sockets: Vec<net::UdpSocket> = Vec::with_capacity(rules.len());

    // vector holding active workers
    let mut active_workers: Vec<ActiveWorker> = Vec::with_capacity(max_workers);

    for i in 0..rules.len() {
        let rule = &rules[i];

        cli::printmsg(&format!("Adding rule: {}", rule.pretty_print()).to_string());

        let bind_socket_addr = net::SocketAddrV4::new(bind_address, rule.listen_port);

        match rule.protocol {
            Protocol::TCP => {
                let listener = net::TcpListener::bind(bind_socket_addr)
                    .expect(format!("Failed to bind to {:?} (TCP)", bind_socket_addr).as_str());
                
                listener.set_nonblocking(true).expect(NON_BLOCKING_FAIL);
                tcp_listeners.push(listener);
            },
            Protocol::UDP => {
                let sock = net::UdpSocket::bind(bind_socket_addr)
                    .expect(format!("Failed to bind to {:?} (UDP)", bind_socket_addr).as_str());

                udp_sockets.push(sock);
            }
        }
    }

    // add workers
    cli::printmsg(&format!("Spawning {} worker(s).", max_workers).to_string());
    for _ in 0..max_workers {
        let worker = create_worker(Arc::clone(&rules));
        active_workers.push(worker);
    }

    // main loop
    loop {
        // check for new tcp connections
        for listener in &tcp_listeners {
            // TODO: don't use a loop, implement a queue of incomming streams to handle
            // TODO: this loop is always infinite in non-blocking mode
            for stream in listener.incoming() {
                match stream {
                    Ok(s) => {
                        let avail_worker_index = match get_idle_worker(&active_workers) {
                            Some(index) => index,
                            None => { continue; } // TODO: this should go back in a queue
                        };

                        let mut avail_worker = active_workers.remove(avail_worker_index);
                        avail_worker.is_idle = false;
                        let res = avail_worker.sender.send(WorkerMsgInbound::HandleTcp(s));
                        match res {
                            Err(s) => {
                                // the worker died, replace it
                                let mut new_worker = create_worker(Arc::clone(&rules));
                                new_worker.is_idle = false;

                                // try again, this time panicking on fail
                                avail_worker.sender.send(s.0)
                                    .expect("Failed twice to pass a message to a worker thread");
                                
                                active_workers.push(new_worker);
                            },

                            _ => {
                                // push the worker back into active_workers
                                active_workers.push(avail_worker);
                            }
                        }
                    },
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        continue;
                    },
                    Err(e) => panic!("Unexpected IO error: {}", e),
                }
            }
        }

        // handle incomming messages from active workers
        for i in 0..active_workers.len() {
            let mut worker = active_workers.remove(i);
            if !worker.is_idle {
                match worker.receiver.try_recv() {
                    Ok(msg) => {
                        match msg {
                            WorkerMsgOutbound::ReturnUdp(udp_sock) => {
                                // return the udp socket to the queue
                                udp_sockets.push(udp_sock);
                            },

                            WorkerMsgOutbound::Done => {
                                // set the idle flag
                                worker.is_idle = true;
                            }
                        }
                    },
                    Err(_) => {}
                }
            }

            active_workers.push(worker);
        }

        thread::sleep(Duration::from_millis(1));
    }

    /*
    // wait for workers
    for worker in active_workers {
       worker.join_handle.join().ok();
    }
    */
}

fn create_worker(rules: Arc<Vec<rules::Rule>>) -> ActiveWorker {
    // create outbound and inbound channels
    let (ob_tx, ob_rx): (mpsc::Sender<WorkerMsgOutbound>, mpsc::Receiver<WorkerMsgOutbound>)
        = mpsc::channel();

    let (ib_tx, ib_rx): (mpsc::Sender<WorkerMsgInbound>, mpsc::Receiver<WorkerMsgInbound>)
        = mpsc::channel();

    Worker::new(rules, ob_tx, ib_rx);

    ActiveWorker {
        sender: ib_tx,
        receiver: ob_rx,
        is_idle: true
    }
}

/// obtain a the index of the next available worker
/// returns None if no workers are available
fn get_idle_worker(workers: &Vec<ActiveWorker>) -> Option<usize> {
    for i in 0..workers.len() {
        let worker = &workers[i];
        if worker.is_idle {
            return Some(i);
        }
    }

    None
}
