use crate::{
    cfg::{get_config, load_config},
    logger::init_logger,
    QuoteData,
};
use log::*;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
        RwLock,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const THREAD_SIZE: usize = 8;
const THREAD_MASK: u64 = (THREAD_SIZE - 1) as u64;

static mut ACTIVE: RwLock<AtomicBool> = RwLock::new(AtomicBool::new(true));

static mut THREAD_VEC: RwLock<Vec<JoinHandle<()>>> = RwLock::new(vec![]);
static mut SENDER_VEC: RwLock<Vec<Sender<QuoteData>>> = RwLock::new(vec![]);

static mut WALKER: RwLock<AtomicU64> = RwLock::new(AtomicU64::new(0));

pub(crate) fn start() -> bool {
    unsafe {
        load_config();
        init_logger(get_config());
        for idx in 0..THREAD_SIZE {
            let (tx, mut rx) = mpsc::channel::<QuoteData>();
            let rs = thread::Builder::new()
                .name(format!("proc-{}", idx))
                .spawn(move || event_handle(&mut rx));
            if rs.is_err() {
                return false;
            }
            let thread = rs.unwrap();
            THREAD_VEC.get_mut().unwrap().push(thread);
            SENDER_VEC.get_mut().unwrap().push(tx);
        }
        info!(
            "{}/{}",
            THREAD_VEC.get_mut().unwrap().len(),
            SENDER_VEC.get_mut().unwrap().len()
        );
        thread::sleep(Duration::from_secs(1));
        return true;
    }
}

pub(crate) fn stop() {
    unsafe {
        ACTIVE.get_mut().unwrap().store(false, Ordering::SeqCst);
        loop {
            if THREAD_VEC
                .get_mut()
                .unwrap()
                .iter()
                .all(|thread| thread.is_finished())
            {
                break;
            } else {
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

pub(crate) fn new_data(data: QuoteData) -> bool {
    unsafe {
        let v = WALKER.get_mut().unwrap().fetch_add(1, Ordering::SeqCst);
        let n = v & THREAD_MASK;
        let walker = AtomicU64::new(0);
        let ret = AtomicBool::new(false);
        debug!(
            "v:{}/n:{}/mask:{}/{}",
            v,
            n,
            THREAD_MASK,
            SENDER_VEC.get_mut().unwrap().len()
        );
        for tx in SENDER_VEC.get_mut().unwrap().iter() {
            let c = walker.fetch_add(1, Ordering::SeqCst);
            debug!("{}/{}", c, n);
            if n == c {
                ret.store(tx.send(data).is_ok(), Ordering::SeqCst);
                break;
            }
        }
        return ret.load(Ordering::SeqCst);
    }
}

fn event_handle(rx: &mut Receiver<QuoteData>) {
    unsafe {
        info!(
            "thread:{} started, active:{}",
            thread::current().name().unwrap(),
            ACTIVE.get_mut().unwrap().load(Ordering::SeqCst)
        );
        const TIMEOUT: Duration = Duration::from_secs(1);
        while ACTIVE.get_mut().unwrap().load(Ordering::SeqCst) {
            let opt = rx.recv_timeout(TIMEOUT);
            if opt.is_ok() {
                let data = opt.unwrap();
                debug!(
                    "Received [{}/{}/{}/{}/{}/{}]",
                    data.id, data.bid, data.ask, data.last, data.volume, data.timestamp
                );
            } else {
                //warn!("recv timeout");
            }
        }
    }
}
