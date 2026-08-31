//! Egress delivery: async callback channel between the engine and callbacks.
//!
//! The engine (data-ingest / window-timer thread) publishes callback events into
//! a bounded single-producer/single-consumer ring. A dedicated `bpe-egress`
//! thread consumes the ring and dispatches to the `FnHolder` callbacks, so user
//! code (Java/Python via FFI, or Rust closures) never runs on the engine thread.
//!
//! Event layout per slot (`EGRESS_SLOT_SIZE` bytes):
//!
//! ```text
//! [fn_selector: u16 (0 = FnHolder id, 1 = FnHolder ptr)]
//! [fn_id_or_ptr: u64]
//! [win_start_ms: i64]
//! [win_end_ms: i64]
//! [size: u64]
//! [step: u64]
//! [payload bytes ...]
//! ```
//!
//! Delivery modes:
//! - `Sync`:  the caller thread invokes the callback directly (legacy behavior).
//! - `Async`: events are copied into the ring; the egress thread dispatches.
//!            If the ring is full, the event is dropped with a warning and a
//!            drop counter (risk-control semantics: drop, never block).
//!
//! The engine calls `publish` on a single thread at a time; the egress thread is
//! the only reader. Ordering is guaranteed per producer (seq-cst store/load on
//! the head/tail cursors).

use crate::{
    callback::{callback, FnHolder},
    param::CallbackParams,
};
use std::{
    alloc::{alloc_zeroed, Layout},
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Mutex, Once,
    },
    thread::{self, JoinHandle},
};

/// Number of slots in the egress ring. Must be a power of two.
const EGRESS_SLOTS: usize = 1024;
/// Bytes per slot: header + payload. Mapper events are `size * record_size`
/// (e.g. LIMIT 10 x 512 B = 5 KiB), so slots must hold at least a full LIMIT
/// batch; larger events (big keyed windows) fall back to synchronous delivery.
const EGRESS_SLOT_SIZE: usize = 8192;
/// Header bytes before the payload in each slot.
const HEADER_SIZE: usize = 48;
/// Max payload bytes copied per event (mapper output is bounded by vec_size,
/// window/keyed outputs are bounded by record counts; larger events fall back
/// to synchronous delivery so no data is silently truncated).
const MAX_ASYNC_PAYLOAD: usize = EGRESS_SLOT_SIZE - HEADER_SIZE;

/// How callbacks are delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    /// Invoke callbacks on the calling (engine) thread - legacy synchronous path.
    Sync,
    /// Publish events to the egress ring; a dedicated thread dispatches them.
    Async,
}

static INIT: Once = Once::new();
static mut EGRESS: Option<EgressRing> = None;
static MODE: AtomicUsize = AtomicUsize::new(0); // 0 = Sync, 1 = Async
static DROPPED: AtomicU64 = AtomicU64::new(0);

struct EgressRing {
    slots: *mut u8,
    /// Producer cursor (next slot to write); single producer.
    head: AtomicU64,
    /// Consumer cursor (next slot to read); single consumer.
    tail: AtomicU64,
    /// Registered FnHolder table (id -> holder). Ids come from a global counter
    /// shared with definitions; each definition registers here exactly once.
    holders: Mutex<Vec<Option<&'static FnHolder>>>,
    handle: Mutex<Option<JoinHandle<()>>>,
    running: AtomicU64,
}

impl EgressRing {
    fn new() -> Self {
        let total = EGRESS_SLOTS * EGRESS_SLOT_SIZE;
        let slots = unsafe { alloc_zeroed(Layout::from_size_align(total, 64).unwrap()) };
        EgressRing {
            slots,
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            holders: Mutex::new(Vec::new()),
            handle: Mutex::new(None),
            running: AtomicU64::new(0),
        }
    }
}

/// Selects the delivery mode. `Async` must be selected before any mapper or
/// aggregate is defined (holders are bound at definition time otherwise).
/// Default is `Sync` (legacy behavior).
pub fn set_delivery_mode(mode: DeliveryMode) {
    MODE.store(
        match mode {
            DeliveryMode::Sync => 0,
            DeliveryMode::Async => 1,
        },
        Ordering::SeqCst,
    );
}

/// Current delivery mode.
pub fn delivery_mode() -> DeliveryMode {
    match MODE.load(Ordering::SeqCst) {
        1 => DeliveryMode::Async,
        _ => DeliveryMode::Sync,
    }
}

/// Number of events dropped because the egress ring was full (Async mode only).
pub fn dropped_events() -> u64 {
    DROPPED.load(Ordering::Relaxed)
}

/// Initializes the egress system lazily. Starts the egress thread when the
/// first Async-mode event is published (or on explicit `init_egress`).
fn lazy_init() -> &'static EgressRing {
    INIT.call_once(|| unsafe {
        EGRESS = Some(EgressRing::new());
    });
    // SAFETY: `EGRESS` is written exactly once (above) before any reader can
    // reach here (INIT.call_once blocks concurrent first callers), and is never
    // mutated afterwards; the Option is always Some from then on.
    unsafe {
        let opt = (&raw const EGRESS).as_ref().unwrap_unchecked();
        opt.as_ref().unwrap_unchecked()
    }
}

/// Starts the egress thread. Idempotent; also called from `start()`.
pub(crate) fn init_egress() {
    let ring = lazy_init();
    if ring.running.load(Ordering::SeqCst) == 0 {
        ring.running.store(1, Ordering::SeqCst);
        let mut handle = ring.handle.lock().unwrap_or_else(|e| e.into_inner());
        if handle.is_none() {
            *handle = thread::Builder::new()
                .name("bpe-egress".to_string())
                .spawn(consume_loop)
                .ok();
        }
    }
}

/// Stops the egress thread (called from `stop()`). The thread exits after
/// draining the ring, so already-published events are still delivered.
pub(crate) fn stop_egress() {
    let ring = lazy_init();
    if ring.running.load(Ordering::SeqCst) != 0 {
        ring.running.store(0, Ordering::SeqCst);
        let mut handle = ring.handle.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(h) = handle.take() {
            let _ = h.join();
        }
    }
}

/// Internal id used to bind a `FnHolder` to the egress table.
static NEXT_HOLDER_ID: AtomicU64 = AtomicU64::new(0);

/// Dispatch one callback event through the current delivery mode.
/// `payload` is the caller's buffer; in Async mode its bytes are copied into
/// the ring slot before this function returns, so callers may reuse the buffer.
/// Events larger than `MAX_ASYNC_PAYLOAD` fall back to synchronous delivery.
pub(crate) fn dispatch(
    holder: &FnHolder,
    u8_ptr: *const u8,
    mask: usize,
    offset: usize,
    size: usize,
    step: usize,
    win_start_ms: i64,
    win_end_ms: i64,
) {
    if delivery_mode() == DeliveryMode::Sync {
        let param = CallbackParams::new_with_window(
            u8_ptr, mask, offset, size, step, win_start_ms, win_end_ms,
        );
        callback(holder, param);
        return;
    }
    let payload_len = size.saturating_mul(step);
    if payload_len > MAX_ASYNC_PAYLOAD {
        // too large to copy: synchronous fallback (never truncate)
        let param = CallbackParams::new_with_window(
            u8_ptr, mask, offset, size, step, win_start_ms, win_end_ms,
        );
        callback(holder, param);
        return;
    }
    let holder_id = register_holder(holder);
    let ring = lazy_init();
    init_egress();
    let head = ring.head.load(Ordering::SeqCst);
    let tail = ring.tail.load(Ordering::SeqCst);
    if head.wrapping_sub(tail) as usize >= EGRESS_SLOTS {
        DROPPED.fetch_add(1, Ordering::Relaxed);
        log::warn!(
            "egress ring full, event dropped (total dropped: {})",
            DROPPED.load(Ordering::Relaxed)
        );
        return;
    }
    let slot = (head % EGRESS_SLOTS as u64) as usize;
    let slot_ptr = unsafe { ring.slots.add(slot * EGRESS_SLOT_SIZE) };
    unsafe {
        // header
        *(slot_ptr as *mut u64) = holder_id;
        *(slot_ptr.add(8) as *mut u64) = size as u64;
        *(slot_ptr.add(16) as *mut u64) = step as u64;
        *(slot_ptr.add(24) as *mut i64) = win_start_ms;
        *(slot_ptr.add(32) as *mut i64) = win_end_ms;
        // payload copy (reuse buffer safety: bytes are owned by the ring now)
        if payload_len > 0 {
            std::ptr::copy_nonoverlapping(u8_ptr, slot_ptr.add(HEADER_SIZE), payload_len);
        }
        // fence: payload written before publishing head
        std::sync::atomic::fence(Ordering::Release);
    }
    ring.head.store(head.wrapping_add(1), Ordering::SeqCst);
}

/// Registers a holder in the egress table and returns its id.
/// Holders are `&'static` leaked by definition paths, so registration is a
/// one-time cost per definition; repeated dispatch reuses the id.
fn register_holder(holder: &FnHolder) -> u64 {
    // pointer identity: fast path - the holder was registered before
    let ring = lazy_init();
    let mut table = ring.holders.lock().unwrap_or_else(|e| e.into_inner());
    let ptr = holder as *const FnHolder;
    for (i, slot) in table.iter().enumerate() {
        if let Some(h) = slot {
            if std::ptr::eq(*h, holder) {
                return i as u64;
            }
        }
    }
    let id = NEXT_HOLDER_ID.fetch_add(1, Ordering::SeqCst);
    let leaked: &'static FnHolder = unsafe { &*(ptr) };
    let idx = id as usize;
    if idx >= table.len() {
        table.resize(idx + 1, None);
    }
    table[idx] = Some(leaked);
    id
}

/// Egress consumer loop: drain the ring, dispatch each event to its holder.
fn consume_loop() {
    let ring = lazy_init();
    let mut spins: u32 = 0;
    loop {
        let tail = ring.tail.load(Ordering::SeqCst);
        let head = ring.head.load(Ordering::SeqCst);
        if tail == head {
            if ring.running.load(Ordering::SeqCst) == 0 {
                return; // stopped and drained
            }
            // adaptive idle: brief spin, then yield
            spins = spins.wrapping_add(1);
            if spins < 100 {
                std::hint::spin_loop();
            } else {
                thread::yield_now();
            }
            continue;
        }
        spins = 0;
        let slot = (tail % EGRESS_SLOTS as u64) as usize;
        let slot_ptr = unsafe { ring.slots.add(slot * EGRESS_SLOT_SIZE) };
        unsafe {
            // fence: read payload only after observing the head advance
            std::sync::atomic::fence(Ordering::Acquire);
            let holder_id = *(slot_ptr as *const u64);
            let size = *(slot_ptr.add(8) as *const u64) as usize;
            let step = *(slot_ptr.add(16) as *const u64) as usize;
            let win_start_ms = *(slot_ptr.add(24) as *const i64);
            let win_end_ms = *(slot_ptr.add(32) as *const i64);
            let payload_ptr = slot_ptr.add(HEADER_SIZE);
            let holder = {
                let table = ring.holders.lock().unwrap_or_else(|e| e.into_inner());
                table.get(holder_id as usize).copied().flatten()
            };
            if let Some(holder) = holder {
                let param = CallbackParams::new_with_window(
                    payload_ptr, 0, 0, size, step, win_start_ms, win_end_ms,
                );
                callback(holder, param);
            } else {
                log::warn!("egress event for unknown holder id [{holder_id}] dropped");
            }
        }
        ring.tail.store(tail.wrapping_add(1), Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn test_egress_async_delivery() {
        use std::sync::Arc;
        set_delivery_mode(DeliveryMode::Async);
        init_egress();
        let count = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&count);
        let holder: &'static FnHolder = Box::leak(Box::new(FnHolder::Func(Box::new(
            move |p: CallbackParams| {
                sink.fetch_add(p.size(), Ordering::SeqCst);
            },
        ))));
        // 3 records * 1 field
        let mut buf = vec![0u8; 3 * 8];
        for i in 0..3 {
            buf[i * 8..(i + 1) * 8].copy_from_slice(&(i as i64).to_le_bytes());
        }
        dispatch(holder, buf.as_ptr(), 0, 0, 3, 8, 0, 0);
        // wait for the egress thread to deliver
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while count.load(Ordering::SeqCst) != 3 {
            if std::time::Instant::now() > deadline {
                panic!("egress delivery timeout");
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        set_delivery_mode(DeliveryMode::Sync);
    }
}
