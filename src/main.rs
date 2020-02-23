mod cli;
mod rules;
mod forwarding;

use std::net::Ipv4Addr;
use std::env;
use std::sync::mpsc;
use forwarding::{Worker, WorkerMsgInbound, WorkerMsgOutbound};

fn main() {
    // bind address is initially localhost
    let mut bind_address = Ipv4Addr::LOCALHOST;

    // parse cmd args
    let rules = cli::parse_args(env::args().collect(), &mut bind_address);
    if rules == None {
        cli::printmsg("No rules specified.");
    }

    let rules = rules.unwrap();

    // spawn workers
    let (tx, rx): (mpsc::Sender<WorkerMsgOutbound>, mpsc::Receiver<WorkerMsgOutbound>) = mpsc::channel();
    let (tx2, rx2): (mpsc::Sender<WorkerMsgInbound>, mpsc::Receiver<WorkerMsgInbound>) = mpsc::channel();
    let worker = Worker::new(&rules, tx, rx2);
    let workers = Vec::<Worker>::with_capacity(4);
}
