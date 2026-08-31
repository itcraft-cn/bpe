//! Egress delivery: unbounded async callback channel between the engine and callbacks.
//!
//! The engine (data-ingest / window-timer threads) publishes callback events into an
//! unbounded MPSC channel (`crossbeam-channel`, lock-free block chain), so events are
//! **never dropped by backpressure**; a dedicated `bpe-egress` thread consumes and
//! dispatches to the `FnHolder` callbacks, so user code never runs on the hot path.
//!
//! ## Timeout watchdog & policies
//!
//! Callback execution time is watched by an independent `bpe-egress-watchdog` thread
//! (100ms tick): the consumer records a processing-start timestamp before each
//! dispatch; if it exceeds the threshold, the watchdog applies the configured policy
//! (the stuck consumer thread itself cannot time out a deadlocked callback):
//!
//! - [`EgressPolicy::Drain`] (1): spawn a sweeper thread that reads and discards all
//!   queued events (counted in `dropped_events`); the stuck thread keeps running.
//! - [`EgressPolicy::Failover`] (2): promote a new consumer thread (generation + 1);
//!   the old thread exits as soon as its callback returns. If the new thread also
//!   hangs (all threads hung), falls back to Drain behavior.
//! - [`EgressPolicy::AlertOnly`] (3): just raise an alert; never interferes.
//!
//! Alerts go to the registered alert listener (FFI, e.g. Java) when present,
//! otherwise default to `log::warn`.
//!
//! ## Delivery modes
//!
//! - `Sync`: legacy direct invocation on the engine thread (Rust users / tests).
//! - `Async`: events are cloned into the channel; the egress thread dispatches.
//!   Oversized payloads (beyond `MAX_ASYNC_PAYLOAD`) still fall back to synchronous
//!   delivery so no data is silently truncated.

use crate::{
    callback::{callback, FnHolder},
    ffi::FfiFunc,
    param::CallbackParams,
};
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::{
    sync::{
        atomic::{AtomicU64, AtomicU8, AtomicBool, Ordering},
        Mutex, Once,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Max payload bytes per async event (oversized -> synchronous fallback).
/// Mapper events are `LIMIT * record_size` (e.g. 10 * 512 B = 5 KiB).
const MAX_ASYNC_PAYLOAD: usize = 64 * 1024;

/// How callbacks are delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    /// Invoke callbacks on the calling (engine) thread - legacy synchronous path.
    Sync,
    /// Publish events to the egress channel; a dedicated thread dispatches them.
    Async,
}

/// What the watchdog does when a callback exceeds the time threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EgressPolicy {
    /// Spawn a sweeper that reads and discards queued events; keep the stuck thread.
    Drain = 1,
    /// Promote a new consumer thread; fall back to Drain when all threads hang.
    Failover = 2,
    /// Only raise alerts (listener / log); never interfere.
    AlertOnly = 3,
}

/// One queued callback event: fixed header + owned payload bytes.
struct EgressEvent {
    holder_id: u64,
    win_start_ms: i64,
    win_end_ms: i64,
    size: usize,
    step: usize,
    payload: Box<[u8]>,
}

static INIT: Once = Once::new();
static mut EGRESS: Option<EgressState> = None;
static MODE: AtomicU8 = AtomicU8::new(0); // 0 = Sync, 1 = Async
static POLICY: AtomicU8 = AtomicU8::new(3); // default AlertOnly
static THRESHOLD_MS: AtomicU64 = AtomicU64::new(100);
static POLICY_DROPPED: AtomicU64 = AtomicU64::new(0);
static ALERTS: AtomicU64 = AtomicU64::new(0);
/// Elapsed ms (from EPOCH_BASE) when the consumer started the current event; 0 = idle.
static PROCESSING_SINCE: AtomicU64 = AtomicU64::new(0);
/// Current consumer generation; a consumer exits when it falls behind.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// Consecutive watchdog timeouts without a completed callback in between.
static HUNG_STREAK: AtomicU64 = AtomicU64::new(0);
static RUNNING: AtomicBool = AtomicBool::new(false);
/// monotonic baseline for PROCESSING_SINCE timestamps
static mut EPOCH_BASE: Option<Instant> = None;

struct EgressState {
    tx: Sender<EgressEvent>,
    rx: Receiver<EgressEvent>,
    holders: Mutex<Vec<Option<&'static FnHolder>>>,
    alert_listener: Mutex<Option<&'static FnHolder>>,
    /// handle of the live consumer (latest generation)
    consumer: Mutex<Option<JoinHandle<()>>>,
    watchdog: Mutex<Option<JoinHandle<()>>>,
}

fn state() -> &'static EgressState {
    INIT.call_once(|| unsafe {
        let (tx, rx) = unbounded();
        EGRESS = Some(EgressState {
            tx,
            rx,
            holders: Mutex::new(Vec::new()),
            alert_listener: Mutex::new(None),
            consumer: Mutex::new(None),
            watchdog: Mutex::new(None),
        });
        EPOCH_BASE = Some(Instant::now());
    });
    // SAFETY: written exactly once inside INIT.call_once before any reader proceeds;
    // never mutated afterwards; the Option is always Some from then on.
    unsafe {
        let opt = (&raw const EGRESS).as_ref().unwrap_unchecked();
        opt.as_ref().unwrap_unchecked()
    }
}

fn now_ms() -> u64 {
    // SAFETY: set with EGRESS in the same call_once (readers only arrive after
    // INIT completes); read-only afterwards, always Some.
    unsafe {
        let opt = (&raw const EPOCH_BASE).as_ref().unwrap_unchecked();
        opt.as_ref().unwrap_unchecked().elapsed().as_millis() as u64
    }
}

/// Selects the delivery mode. Default is `Sync` (legacy behavior); the Java/Python
/// bindings switch to `Async` on start.
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

/// Configures the watchdog policy and callback time threshold.
/// Unknown ids keep the current policy. Default: AlertOnly / 100 ms.
pub fn set_egress_policy(policy: EgressPolicy, threshold_ms: u64) {
    POLICY.store(policy as u8, Ordering::SeqCst);
    THRESHOLD_MS.store(threshold_ms.max(1), Ordering::SeqCst);
}

/// Number of events waiting in the egress channel (async mode).
pub fn pending_events() -> usize {
    state().rx.len()
}

/// Number of events discarded by the Drain policy (async mode). The unbounded
/// channel itself never drops events.
pub fn dropped_events() -> u64 {
    POLICY_DROPPED.load(Ordering::Relaxed)
}

/// Number of timeout alerts raised so far.
pub fn alert_count() -> u64 {
    ALERTS.load(Ordering::Relaxed)
}

/// Registers (or clears with `None`) an alert listener invoked on watchdog
/// timeouts. The listener receives a CallbackParams payload laid out as
/// `[elapsed_ms: i64][pending: i64][dropped: i64][policy: i64]`, size=1, step=32.
pub fn set_alert_listener(listener: Option<Box<dyn FfiFunc>>) {
    let leaked = listener.map(|ffi| -> &'static FnHolder { Box::leak(Box::new(FnHolder::FfiFunc(ffi))) });
    let mut slot = state().alert_listener.lock().unwrap_or_else(|e| e.into_inner());
    *slot = leaked;
}

/// Starts the egress consumer and watchdog threads. Idempotent; also called
/// from `start()`.
pub(crate) fn init_egress() {
    if !RUNNING.swap(true, Ordering::SeqCst) {
        let st = state();
        // consumer
        {
            let mut h = st.consumer.lock().unwrap_or_else(|e| e.into_inner());
            if h.is_none() {
                let gen = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
                *h = thread::Builder::new()
                    .name("bpe-egress".to_string())
                    .spawn(move || consume_loop(gen))
                    .ok();
            }
        }
        // watchdog
        {
            let mut h = st.watchdog.lock().unwrap_or_else(|e| e.into_inner());
            if h.is_none() {
                *h = thread::Builder::new()
                    .name("bpe-egress-watchdog".to_string())
                    .spawn(watchdog_loop)
                    .ok();
            }
        }
    }
}

/// Stops the egress threads. The watchdog joins; a healthy consumer drains the
/// remaining events then exits. A consumer stuck in a user callback cannot be
/// joined - it is detached and exits by itself once the callback returns.
pub(crate) fn stop_egress() {
    if !RUNNING.swap(false, Ordering::SeqCst) {
        return;
    }
    let st = state();
    let wd = st
        .watchdog
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    if let Some(h) = wd {
        let _ = h.join();
    }
    let consumer = st
        .consumer
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    if let Some(h) = consumer {
        // bounded wait: a stuck consumer must not block stop()
        let deadline = Instant::now() + Duration::from_millis(500);
        while !h.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
    }
    PROCESSING_SINCE.store(0, Ordering::SeqCst);
    HUNG_STREAK.store(0, Ordering::SeqCst);
}

/// Dispatch one callback event through the current delivery mode.
/// In Async mode the payload bytes are moved into the channel, so callers may
/// reuse their buffer as soon as this returns.
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
        invoke_sync(holder, u8_ptr, mask, offset, size, step, win_start_ms, win_end_ms);
        return;
    }
    let payload_len = size.saturating_mul(step);
    if payload_len > MAX_ASYNC_PAYLOAD {
        // too large to clone: synchronous fallback (never truncate)
        invoke_sync(holder, u8_ptr, mask, offset, size, step, win_start_ms, win_end_ms);
        return;
    }
    let st = state();
    init_egress();
    let holder_id = register_holder(holder);
    let mut buf = vec![0_u8; payload_len].into_boxed_slice();
    unsafe {
        if payload_len > 0 {
            std::ptr::copy_nonoverlapping(u8_ptr, buf.as_mut_ptr(), payload_len);
        }
    }
    let _ = st.tx.send(EgressEvent {
        holder_id,
        win_start_ms,
        win_end_ms,
        size,
        step,
        payload: buf,
    });
}

fn invoke_sync(
    holder: &FnHolder,
    u8_ptr: *const u8,
    mask: usize,
    offset: usize,
    size: usize,
    step: usize,
    win_start_ms: i64,
    win_end_ms: i64,
) {
    let param =
        CallbackParams::new_with_window(u8_ptr, mask, offset, size, step, win_start_ms, win_end_ms);
    callback(holder, param);
}

/// Registers a holder and returns its stable id (pointer-identity cache).
fn register_holder(holder: &FnHolder) -> u64 {
    let st = state();
    let mut table = st.holders.lock().unwrap_or_else(|e| e.into_inner());
    for (i, slot) in table.iter().enumerate() {
        if let Some(h) = slot {
            if std::ptr::eq(*h, holder) {
                return i as u64;
            }
        }
    }
    // SAFETY: holders live in engine-global registration tables for the process
    // lifetime (leaked at definition time), so extending the lifetime here is sound.
    let leaked: &'static FnHolder = unsafe { &*(holder as *const FnHolder) };
    table.push(Some(leaked));
    (table.len() - 1) as u64
}

/// Egress consumer loop. Exits when stopped or when superseded by a newer
/// generation (Failover policy).
fn consume_loop(gen: u64) {
    let st = state();
    let mut spins: u32 = 0;
    loop {
        if !RUNNING.load(Ordering::SeqCst) || GENERATION.load(Ordering::SeqCst) != gen {
            return;
        }
        match st.rx.try_recv() {
            Ok(ev) => {
                spins = 0;
                PROCESSING_SINCE.store(now_ms().max(1), Ordering::SeqCst);
                dispatch_event(&ev);
                PROCESSING_SINCE.store(0, Ordering::SeqCst);
                HUNG_STREAK.store(0, Ordering::SeqCst);
            }
            Err(_) => {
                spins = spins.wrapping_add(1);
                if spins < 100 {
                    std::hint::spin_loop();
                } else {
                    thread::yield_now();
                }
            }
        }
    }
}

fn dispatch_event(ev: &EgressEvent) {
    let st = state();
    let holder = {
        let table = st.holders.lock().unwrap_or_else(|e| e.into_inner());
        table.get(ev.holder_id as usize).copied().flatten()
    };
    if let Some(holder) = holder {
        let param = CallbackParams::new_with_window(
            ev.payload.as_ptr(),
            0,
            0,
            ev.size,
            ev.step,
            ev.win_start_ms,
            ev.win_end_ms,
        );
        callback(holder, param);
    } else {
        log::warn!("egress event for unknown holder id [{}] dropped", ev.holder_id);
    }
}

/// Watchdog loop: checks the current event's processing time every tick and
/// applies the configured policy on timeout.
fn watchdog_loop() {
    const TICK_MS: u64 = 20;
    loop {
        if !RUNNING.load(Ordering::SeqCst) {
            return;
        }
        thread::sleep(Duration::from_millis(TICK_MS));
        let since = PROCESSING_SINCE.load(Ordering::SeqCst);
        if since == 0 {
            continue;
        }
        let elapsed = now_ms().saturating_sub(since);
        let threshold = THRESHOLD_MS.load(Ordering::SeqCst);
        if elapsed < threshold {
            continue;
        }
        let streak = HUNG_STREAK.fetch_add(1, Ordering::SeqCst) + 1;
        ALERTS.fetch_add(1, Ordering::Relaxed);
        let pending = state().rx.len() as i64;
        let policy = POLICY.load(Ordering::SeqCst);
        fire_alert(elapsed as i64, pending, policy as i64);
        match policy {
            1 => spawn_sweeper(),
            2 => {
                if streak >= 2 {
                    // every consumer thread is hung: fall back to Drain
                    spawn_sweeper();
                } else {
                    promote_new_consumer();
                }
            }
            _ => {}
        }
        // back off so one hung event does not fire the policy every tick
        PROCESSING_SINCE.store(now_ms().max(1) - threshold, Ordering::SeqCst);
    }
}

/// Drain policy: spawn a sweeper that reads and discards queued events.
fn spawn_sweeper() {
    let spawned = thread::Builder::new()
        .name("bpe-egress-sweeper".to_string())
        .spawn(|| {
            let rx = &state().rx;
            let mut n: u64 = 0;
            while let Ok(_ev) = rx.try_recv() {
                n += 1;
            }
            if n > 0 {
                POLICY_DROPPED.fetch_add(n, Ordering::Relaxed);
                log::warn!("egress sweeper discarded {n} queued events");
            }
        })
        .ok();
    if spawned.is_none() {
        log::warn!("failed to spawn egress sweeper");
    }
}

/// Failover policy: promote a new consumer generation.
fn promote_new_consumer() {
    let gen = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let st = state();
    let _h = st.consumer.lock().unwrap_or_else(|e| e.into_inner());
    let spawned = thread::Builder::new()
        .name(format!("bpe-egress-{gen}"))
        .spawn(move || consume_loop(gen))
        .ok();
    if spawned.is_some() {
        log::warn!("egress consumer promoted to generation {gen}");
    }
    // old handle is intentionally not joined: the thread exits by itself once
    // its (stuck) callback returns and it observes the generation change
}

/// Fires the alert: listener first, log fallback.
fn fire_alert(elapsed_ms: i64, pending: i64, policy: i64) {
    let st = state();
    let listener = {
        let slot = st.alert_listener.lock().unwrap_or_else(|e| e.into_inner());
        *slot
    };
    let dropped = POLICY_DROPPED.load(Ordering::Relaxed) as i64;
    let info = [
        elapsed_ms.to_le_bytes(),
        pending.to_le_bytes(),
        dropped.to_le_bytes(),
        policy.to_le_bytes(),
    ]
    .concat();
    match listener {
        Some(holder) => {
            let param = CallbackParams::new(info.as_ptr(), 0, 0, 1, 32);
            callback(holder, param);
        }
        None => {
            log::warn!(
                "egress callback timeout: elapsed={elapsed_ms}ms pending={pending} dropped={dropped} policy={policy}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::param::CallbackParams;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    fn leak_holder(f: impl Fn(CallbackParams) + Send + 'static) -> &'static FnHolder {
        Box::leak(Box::new(FnHolder::Func(Box::new(f))))
    }

    fn wait_until(timeout_ms: u64, cond: impl Fn() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        while !cond() {
            if Instant::now() > deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(2));
        }
        true
    }

    /// All egress scenarios share global engine state (one channel, one consumer),
    /// so they must run sequentially inside a single test.
    #[test]
    fn test_egress_end_to_end() {
        // -- 1. basic async delivery ----------------------------------------
        set_delivery_mode(DeliveryMode::Async);
        init_egress();
        let count = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&count);
        let holder = leak_holder(move |p: CallbackParams| {
            sink.fetch_add(p.size(), Ordering::SeqCst);
        });
        let mut buf = vec![0u8; 3 * 8];
        for i in 0..3 {
            buf[i * 8..(i + 1) * 8].copy_from_slice(&(i as i64).to_le_bytes());
        }
        dispatch(holder, buf.as_ptr(), 0, 0, 3, 8, 0, 0);
        assert!(
            wait_until(5000, || count.load(Ordering::SeqCst) == 3),
            "egress delivery timeout"
        );

        // -- 2. unbounded: no loss under a large burst ----------------------
        let total = 20_000_usize;
        for _ in 0..total {
            dispatch(holder, buf.as_ptr(), 0, 0, 1, 8, 0, 0);
        }
        assert!(
            wait_until(10_000, || count.load(Ordering::SeqCst) == 3 + total),
            "unbounded delivery lost events: {}/{}",
            count.load(Ordering::SeqCst),
            3 + total
        );
        assert_eq!(dropped_events(), 0, "unbounded channel must not drop");

        // -- 3. Drain policy: sweeper discards backlog on timeout ------------
        set_egress_policy(EgressPolicy::Drain, 60);
        let gate = Arc::new(AtomicU8::new(1)); // 1: slow, 0: fast
        let g = Arc::clone(&gate);
        let slow = leak_holder(move |_: CallbackParams| {
            if g.load(Ordering::SeqCst) == 1 {
                thread::sleep(Duration::from_millis(400)); // exceeds the threshold
            }
        });
        // first event hangs the consumer; more events pile up behind it
        dispatch(slow, buf.as_ptr(), 0, 0, 1, 8, 0, 0);
        for _ in 0..50 {
            dispatch(slow, buf.as_ptr(), 0, 0, 1, 8, 0, 0);
        }
        assert!(
            wait_until(5000, || alert_count() >= 1),
            "watchdog did not alert"
        );
        assert!(
            wait_until(5000, || dropped_events() >= 1),
            "drain policy did not discard backlog"
        );
        gate.store(0, Ordering::SeqCst); // release the stuck callback
        assert!(wait_until(5000, || pending_events() == 0));
        // wait until the consumer actually returns from the stuck callback
        assert!(
            wait_until(5000, || PROCESSING_SINCE.load(Ordering::SeqCst) == 0),
            "consumer still processing after drain scenario"
        );

        // -- 4. Failover policy: new consumer generation on timeout ----------
        set_egress_policy(EgressPolicy::Failover, 60);
        HUNG_STREAK.store(0, Ordering::SeqCst); // reset the Drain-scenario streak
        let gen0 = GENERATION.load(Ordering::SeqCst);
        let gate2 = Arc::new(AtomicU8::new(1));
        let g2 = Arc::clone(&gate2);
        let slow2 = leak_holder(move |_: CallbackParams| {
            if g2.load(Ordering::SeqCst) == 1 {
                thread::sleep(Duration::from_millis(800));
            }
        });
        dispatch(slow2, buf.as_ptr(), 0, 0, 1, 8, 0, 0);
        assert!(
            wait_until(5000, || GENERATION.load(Ordering::SeqCst) > gen0),
            "failover did not promote a new consumer"
        );
        gate2.store(0, Ordering::SeqCst); // let the stuck one finish & exit
        assert!(wait_until(5000, || pending_events() == 0));

        // -- restore defaults ------------------------------------------------
        set_egress_policy(EgressPolicy::AlertOnly, 100);
        set_delivery_mode(DeliveryMode::Sync);
    }
}
