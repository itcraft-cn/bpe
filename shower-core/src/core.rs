use crate::{
    cfg::{get_config, load_config},
    logger::init_logger,
    QuoteTick,
};
use log::*;
use multiqueue::{mpmc_queue, MPMCReceiver, MPMCSender};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        PoisonError, RwLock,
    },
    thread::{self, JoinHandle},
};

static mut ACTIVE: RwLock<AtomicBool> = RwLock::new(AtomicBool::new(true));
static mut OPT_SENDER: Option<MPMCSender<QuoteTick>> = None;
static mut OPT_RECV_TH: Option<JoinHandle<()>> = None;

pub fn start() -> bool {
    load_config();
    init_logger(get_config());

    let (sender, receiver) = mpmc_queue(65536);

    unsafe {
        OPT_SENDER.get_or_insert(sender);
    }

    let rs = thread::Builder::new()
        .name(String::from("receiver"))
        .spawn(move || handle_recv(receiver));

    if let Ok(recv_th) = rs {
        unsafe {
            OPT_RECV_TH.get_or_insert(recv_th);
        }
        true
    } else {
        false
    }
}

fn handle_recv(receiver: MPMCReceiver<QuoteTick>) {
    let _cfg = get_config();
    let walker = AtomicU64::new(0);
    loop {
        let rs = receiver.try_recv();
        if let Ok(tick) = rs {
            debug!("Receiving data from queue: {:?}", tick);
            // let now = walker.load(Ordering::SeqCst);
            // if now % 100000 == 0 {
            //     info!("Received {} ticks", now);
            // }
            walker.fetch_add(1, Ordering::SeqCst);
        }
        if !check_active() {
            let now = walker.load(Ordering::SeqCst);
            info!("finally, received {} ticks", now);
            break;
        }
    }
}

fn check_active() -> bool {
    unsafe {
        ACTIVE
            .get_mut()
            .unwrap_or_else(handle_fetch_error_ret_fake)
            .load(Ordering::SeqCst)
    }
}

fn handle_fetch_error_ret_fake(e: PoisonError<&mut AtomicBool>) -> &mut AtomicBool {
    static mut FAKE: AtomicBool = AtomicBool::new(false);
    warn!("Failed to read ACTIVE: {:?}", e);
    unsafe { &mut FAKE }
}

pub fn stop() {
    unsafe {
        ACTIVE
            .get_mut()
            .unwrap_or_else(handle_fetch_error)
            .store(false, Ordering::SeqCst);
    }
    wait_receiver_quit();
}

fn wait_receiver_quit() {
    info!("mark as deactived");
    unsafe {
        loop {
            if OPT_RECV_TH.as_ref().unwrap().is_finished() {
                info!("receiver thread is finished");
                break;
            }
            info!("receiver thread is running, waiting for it");
            thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

fn handle_fetch_error(e: PoisonError<&mut AtomicBool>) -> &mut AtomicBool {
    warn_and_panic!("fail to set active to false, cannot stop threads:{}", e);
}

pub fn new_data(data: QuoteTick) -> bool {
    unsafe {
        if let Some(sender) = OPT_SENDER.as_ref() {
            let rs = sender.try_send(data);
            if let Ok(_res) = rs {
                debug!("Sending data to queue: {:?}", data);
            } else {
                warn!(
                    "Failed to send data to queue: {:?}, {}",
                    data,
                    rs.err().unwrap()
                );
            }
        }
    }
    true
}

pub fn def_action(sql: &str) {
    info!("{}", sql);
}
