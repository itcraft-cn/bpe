use crate::{
    cfg::{get_config, load_config},
    logger::init_logger,
};
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const ACTIVE: AtomicBool = AtomicBool::new(false);
const THREAD_VEC: RefCell<Vec<JoinHandle<()>>> = RefCell::new(vec![]);
const SENDER_VEC: RefCell<Vec<Sender<[u8; 8192]>>> = RefCell::new(vec![]);
const THREAD_SIZE: usize = 8;
const THREAD_MASK: usize = THREAD_SIZE - 1;
const WALKER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn start() -> bool {
    load_config();
    init_logger(get_config());
    ACTIVE.store(true, Ordering::SeqCst);
    for idx in 0..THREAD_SIZE {
        let (tx, mut rx) = mpsc::channel::<[u8; 8192]>();
        let rs = thread::Builder::new()
            .name(format!("proc-{}", idx))
            .spawn(move || event_handle(&mut rx));
        if rs.is_err() {
            return false;
        }
        let thread = rs.unwrap();
        THREAD_VEC.borrow_mut().push(thread);
        SENDER_VEC.borrow_mut().push(tx);
    }
    return true;
}

pub(crate) fn stop() {
    ACTIVE.store(false, Ordering::SeqCst);
    loop {
        if THREAD_VEC
            .borrow_mut()
            .iter()
            .all(|thread| thread.is_finished())
        {
            break;
        } else {
            thread::sleep(Duration::from_millis(100));
        }
    }
}

pub(crate) fn new_data(data: [u8; 8192]) -> bool {
    let n = WALKER.fetch_add(1, Ordering::SeqCst) & THREAD_MASK as u64;
    let walker = AtomicU8::new(0);
    let ret = AtomicBool::new(false);
    SENDER_VEC.borrow_mut().iter().for_each(|tx| {
        let c = walker.fetch_add(1, Ordering::SeqCst);
        if n as u8 == c {
            ret.store(tx.send(data).is_ok(), Ordering::SeqCst);
        }
    });
    return ret.load(Ordering::SeqCst);
}

fn event_handle(rx: &mut Receiver<[u8; 8192]>) {
    while ACTIVE.load(Ordering::SeqCst) {
        const TIMEOUT: Duration = Duration::from_secs(1);
        let opt = rx.recv_timeout(TIMEOUT);
        if opt.is_ok() {
            let _data = opt.unwrap();
        } else {
            println!("recv timeout");
        }
    }
}
