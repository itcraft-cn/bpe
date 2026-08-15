//! Time-window support for risk-control / alerting style aggregations.
//!
//! Two windowing modes are provided on top of the existing single-record filter:
//! - `Tumbling` (fixed window): every `period_ms` a new window starts; when a
//!   window's end time passes (plus a `lag_ms` tolerance), its aggregated result
//!   is delivered to the callback with `CallbackParams::window_start_ms/end_ms`.
//! - `Sliding` (hopping window): windows of `length_ms` advanced by `slide_ms`;
//!   a record may belong to several overlapping windows.
//!
//! Event time comes from a Long (milliseconds) column (`ts_field`), falling back
//! to processing time when no column is configured. In event-time mode a
//! watermark (max observed event time minus `lag_ms`) drives early firing and
//! drops records that arrive later than the watermark (out-of-order tolerance).
//! The wall clock is kept as a fallback so idle windows still fire.
//!
//! Optional per-key grouping (`key_field`): the window result is delivered once
//! per key, rows are `[key i64][field0]...[fieldN]` with `size()=#keys`.
//!
//! Threading: window buckets and the registry are guarded by `WINDOW_LOCK`; a
//! daemon timer thread (started on the first window definition) scans for due
//! windows every `TICK_MS`. Aggregate computation runs outside the lock on a
//! per-instance result buffer, so the regular (window-less) hot path is
//! untouched and never takes the lock.

use crate::{
    aggregate::{compute_data, gen_aggregate, init_data, WrappedAggregate},
    aux::fetch_ptr,
    callback::{callback, FnHolder},
    consts::FIELD_SIZE,
    data::{Record, U8Bytes},
    id::next_window_id,
    jit::aggregate::{gen_aggregate_func, AggFunc},
    param::CallbackParams,
    sql::{
        base::{parse_options, FilterFunc, ParsedSql},
        select::parse_select,
    },
    store::get_record_size,
};
use globalvar::{def_global_ptr, get_global, get_global_mut};
use hashbrown::HashMap;
use inkwell::execution_engine::JitFunction;
use std::{
    alloc::{self, Layout},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, Once,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Window mode of a window aggregate.
#[derive(Debug, Clone, Copy)]
pub enum Window {
    /// No window: not usable with `def_window_aggregate`.
    None,
    /// Fixed (tumbling) window: one window per `period_ms`.
    Tumbling { period_ms: u64 },
    /// Sliding (hopping) window: `length_ms` long, advanced by `slide_ms`.
    Sliding { length_ms: u64, slide_ms: u64 },
}

/// Timer resolution: how often due windows are checked.
const TICK_MS: u64 = 20;
/// Size of the aggregate result/state area per instance (see aggregate.rs).
const AGG_BUF_SIZE: usize = 8192;
/// Per-key working buffer: 4KB state area + room for many fields.
const KEY_BUF_SIZE: usize = 8192;

static mut PTR_WINDOW_AGGS: u64 = 0;
static mut PTR_PARSE_OPTIONS: u64 = 0;
static WINDOW_LOCK: Mutex<()> = Mutex::new(());
static TIMER_RUNNING: AtomicBool = AtomicBool::new(false);
static TIMER_HANDLE: Mutex<Option<thread::JoinHandle<()>>> = Mutex::new(None);

fn init_window() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        PTR_WINDOW_AGGS = def_global_ptr(Vec::<&'static mut WindowAggregate>::new());
        PTR_PARSE_OPTIONS = def_global_ptr(parse_options());
    });
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Euclidean-style floor division for possibly negative timestamps.
fn div_floor(a: i64, b: i64) -> i64 {
    let q = a / b;
    if a % b != 0 && (a < 0) != (b < 0) {
        q - 1
    } else {
        q
    }
}

/// A time bucket holding filtered records (dense, `record_size` aligned).
struct WindowBucket {
    start_ms: i64,
    end_ms: i64,
    records: Vec<u8>,
}

impl WindowBucket {
    fn new(start_ms: i64, end_ms: i64) -> Self {
        Self {
            start_ms,
            end_ms,
            records: Vec::with_capacity(get_record_size()),
        }
    }
}

/// One registered window aggregate: filter + aggregate executors + buckets.
pub(crate) struct WindowAggregate {
    wrapped: WrappedAggregate,
    filter: JitFunction<'static, FilterFunc>,
    ts_offset: Option<usize>,
    /// Per-key grouping column offset (None = window-level aggregation).
    key_offset: Option<usize>,
    window: Window,
    lag_ms: u64,
    buckets: Vec<WindowBucket>,
    result_buf: *mut u8,
    stream_offsets: Vec<usize>,
    step_total: usize,
    /// Max observed event time, drives the event-time watermark.
    max_seen_ts: i64,
    /// JIT-compiled aggregate kernel (None = fall back to interpreter).
    agg_jit: Option<JitFunction<'static, AggFunc>>,
}

/// Defines a window aggregate (window-level result).
pub(crate) fn define_window_aggregate(
    sql: &str,
    window: Window,
    ts_field: Option<&str>,
    lag_ms: u64,
    func_holder: FnHolder,
) -> Option<u16> {
    define_window_aggregate_impl(sql, window, ts_field, None, lag_ms, func_holder)
}

/// Defines a per-key window aggregate (one result row per key).
pub(crate) fn define_keyed_window_aggregate(
    sql: &str,
    window: Window,
    ts_field: Option<&str>,
    key_field: &str,
    lag_ms: u64,
    func_holder: FnHolder,
) -> Option<u16> {
    define_window_aggregate_impl(sql, window, ts_field, Some(key_field), lag_ms, func_holder)
}

fn define_window_aggregate_impl(
    sql: &str,
    window: Window,
    ts_field: Option<&str>,
    key_field: Option<&str>,
    lag_ms: u64,
    func_holder: FnHolder,
) -> Option<u16> {
    if matches!(window, Window::None) {
        log::warn!("window aggregate requires a real window (Tumbling/Sliding)");
        return None;
    }
    init_window();
    let options = get_global(unsafe { PTR_PARSE_OPTIONS });
    let parsed: ParsedSql = parse_select(sql, options)?;
    let aggregate = gen_aggregate(&parsed).ok()?;
    let stream_id = aggregate.stream_id();
    let stream = Record::get_record(stream_id)?;
    let ts_offset = match ts_field {
        Some(name) => Record::fetch_column_id(stream_id, name).map(|cid| stream.column(*cid).offset()),
        None => None,
    };
    let key_offset = match key_field {
        Some(name) => Some(stream.column(*Record::fetch_column_id(stream_id, name)?).offset()),
        None => None,
    };
    let filter = parsed.filter().clone();
    let stream_offsets = build_stream_offsets(stream);
    let step_total = aggregate.executors().len() * FIELD_SIZE;
    let result_buf = unsafe { alloc::alloc(Layout::from_size_align(AGG_BUF_SIZE, 1).unwrap()) };
    let agg_jit = gen_aggregate_func(&aggregate, &stream_offsets);
    let agg = Box::leak(Box::new(WindowAggregate {
        wrapped: WrappedAggregate::new(aggregate, func_holder),
        filter,
        ts_offset,
        key_offset,
        window,
        lag_ms,
        buckets: vec![],
        result_buf,
        stream_offsets,
        step_total,
        max_seen_ts: i64::MIN,
        agg_jit,
    }));
    {
        let _guard = WINDOW_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let aggs = get_global_mut::<Vec<&'static mut WindowAggregate>>(unsafe { PTR_WINDOW_AGGS });
        aggs.push(agg);
    }
    start_timer();
    Some(next_window_id())
}

/// Builds the stream-column-offset table used by the aggregate input reader:
/// the window input records are raw records (record layout), so each stream
/// field id maps directly to its column offset.
fn build_stream_offsets(stream: &Record) -> Vec<usize> {
    let ncols = stream._columns().len();
    let mut offsets = vec![usize::MAX; ncols + 1];
    for cid in 1..=ncols {
        offsets[cid] = stream.column(cid as u16).offset();
    }
    offsets
}

/// Entry point called from `new_data` (after the regular mapper path).
/// Cheap fast path when no window aggregate is registered.
pub(crate) fn on_new_data(record_id: u16, data: &U8Bytes) {
    init_window();
    let has_agg = {
        let aggs = get_global::<Vec<&'static mut WindowAggregate>>(unsafe { PTR_WINDOW_AGGS });
        !aggs.is_empty()
    };
    if !has_agg {
        return;
    }
    let _guard = WINDOW_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let aggs = get_global_mut::<Vec<&'static mut WindowAggregate>>(unsafe { PTR_WINDOW_AGGS });
    for agg in aggs.iter_mut() {
        if agg.wrapped.aggregate().stream_id() == record_id {
            agg.add_record(data);
        }
    }
}

impl WindowAggregate {
    /// Reads the event timestamp: the configured ts column, or processing time.
    fn event_ts(&self, data: &U8Bytes) -> i64 {
        match self.ts_offset {
            Some(off) => unsafe { fetch_ptr::<i64>(data.bytes().as_ptr().add(off)) },
            None => now_ms(),
        }
    }

    /// Event-time watermark: max observed ts minus the out-of-order tolerance.
    /// Only meaningful in event-time mode.
    fn watermark(&self) -> i64 {
        if self.ts_offset.is_some() {
            self.max_seen_ts - self.lag_ms as i64
        } else {
            i64::MIN
        }
    }

    /// The half-open windows [start, end) covering a timestamp, aligned to the
    /// window definition. For sliding windows every record may cover several
    /// windows; the first start satisfies `s > ts - length` (i.e. s + length > ts).
    fn covering_windows(&self, ts: i64) -> Vec<(i64, i64)> {
        match self.window {
            Window::Tumbling { period_ms } => {
                let period = period_ms as i64;
                let end = div_floor(ts, period) * period + period;
                vec![(end - period, end)]
            }
            Window::Sliding { length_ms, slide_ms } => {
                let length = length_ms as i64;
                let slide = slide_ms as i64;
                let first = div_floor(ts - length, slide) * slide + slide;
                let last = div_floor(ts, slide) * slide;
                let mut out = vec![];
                let mut s = first;
                while s <= last {
                    out.push((s, s + length));
                    s += slide;
                }
                out
            }
            Window::None => vec![],
        }
    }

    /// Filters the record (JIT) and places it into every covering time bucket.
    /// Empty buckets are created even when the filter rejects the record, so an
    /// empty window still fires (with size 0) once its end time passes.
    /// Records older than the watermark are dropped (out-of-order tolerance).
    fn add_record(&mut self, data: &U8Bytes) {
        let ts = self.event_ts(data);
        if ts > self.max_seen_ts {
            self.max_seen_ts = ts;
        }
        let v_ptr = data.bytes().as_ptr() as u64;
        let matched = unsafe { self.filter.call(v_ptr) };
        let watermark = self.watermark();
        for (start, end) in self.covering_windows(ts) {
            if end < watermark {
                continue; // late record beyond the watermark: dropped
            }
            if matched {
                self.add_to_bucket(start, end, data);
            } else {
                self.ensure_bucket(start, end);
            }
        }
    }

    fn ensure_bucket(&mut self, start_ms: i64, end_ms: i64) {
        if !self
            .buckets
            .iter()
            .any(|b| b.start_ms == start_ms && b.end_ms == end_ms)
        {
            self.buckets.push(WindowBucket::new(start_ms, end_ms));
        }
    }

    fn add_to_bucket(&mut self, start_ms: i64, end_ms: i64, data: &U8Bytes) {
        let rec_size = get_record_size();
        let len = data.data_len().min(rec_size);
        let src = data.bytes();
        if let Some(b) = self
            .buckets
            .iter_mut()
            .find(|b| b.start_ms == start_ms && b.end_ms == end_ms)
        {
            let base = b.records.len();
            b.records.resize(base + rec_size, 0);
            b.records[base..base + len].copy_from_slice(&src[..len]);
        } else {
            let mut b = WindowBucket::new(start_ms, end_ms);
            let base = b.records.len();
            b.records.resize(base + rec_size, 0);
            b.records[base..base + len].copy_from_slice(&src[..len]);
            self.buckets.push(b);
        }
    }

    fn window_max_length(&self) -> u64 {
        match self.window {
            Window::Tumbling { period_ms } => period_ms,
            Window::Sliding { length_ms, .. } => length_ms,
            Window::None => 0,
        }
    }
}

/// Timer thread body: every tick, collect due buckets under the lock, then
/// compute the aggregates outside the lock and deliver the callbacks.
struct Pending {
    agg: *const WindowAggregate,
    start_ms: i64,
    end_ms: i64,
    records: Vec<u8>,
}

fn check_and_trigger() {
    let mut pending: Vec<Pending> = vec![];
    {
        let _guard = WINDOW_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let now = now_ms();
        let aggs = get_global_mut::<Vec<&'static mut WindowAggregate>>(unsafe { PTR_WINDOW_AGGS });
        for agg_ref in aggs.iter_mut() {
            let agg_ptr = *agg_ref as *const WindowAggregate;
            let agg_mut: &mut WindowAggregate = &mut **agg_ref;
            let watermark = agg_mut.watermark();
            let mut i = 0;
            while i < agg_mut.buckets.len() {
                // event-time watermark OR wall-clock fallback
                let due = agg_mut.buckets[i].end_ms < watermark
                    || agg_mut.buckets[i].end_ms + agg_mut.lag_ms as i64 <= now;
                if due {
                    let b = agg_mut.buckets.remove(i);
                    pending.push(Pending {
                        agg: agg_ptr,
                        start_ms: b.start_ms,
                        end_ms: b.end_ms,
                        records: b.records,
                    });
                } else {
                    i += 1;
                }
            }
            // bound memory: drop buckets that have long passed their window
            let keep = now - agg_mut.window_max_length() as i64 * 2 - 1000;
            agg_mut.buckets.retain(|b| b.end_ms >= keep);
        }
    }
    // compute and deliver outside the lock
    for p in pending {
        deliver_window(&p);
    }
}

/// Computes the aggregate result for one due window and invokes the callback.
/// Window-level: one result row. Per-key: one row per key.
fn deliver_window(p: &Pending) {
    let agg = unsafe { &*p.agg };
    let count = p.records.len() / get_record_size();
    let stream = match Record::get_record(agg.wrapped.aggregate().stream_id()) {
        Some(s) => s,
        None => return,
    };
    if let Some(key_off) = agg.key_offset {
        deliver_keyed(p, agg, stream, count, key_off);
    } else {
        let buf = agg.result_buf;
        match &agg.agg_jit {
            Some(jit) => {
                // JIT kernel: computes init for count==0 and results otherwise
                unsafe { jit.call(p.records.as_ptr(), count, get_record_size(), buf) };
            }
            None => {
                if !init_data(buf, &agg.wrapped) {
                    return;
                }
                if count > 0 {
                    let param =
                        CallbackParams::new(p.records.as_ptr(), 0, 0, count, get_record_size());
                    compute_data(buf, &agg.wrapped, stream, param, &agg.stream_offsets);
                }
            }
        }
        let size = if count > 0 { 1 } else { 0 };
        let param = CallbackParams::new_with_window(
            buf,
            0,
            0,
            size,
            agg.step_total,
            p.start_ms,
            p.end_ms,
        );
        callback(agg.wrapped.fn_holder(), param);
    }
}

/// Per-key delivery: group the window records by the key column, compute the
/// aggregate per group into a row `[key i64][field0]...[fieldN]`, and deliver
/// all rows in one callback (`size()=#keys`, `step()=(1+N)*8`).
fn deliver_keyed(p: &Pending, agg: &WindowAggregate, stream: &Record, count: usize, key_off: usize) {
    let rec_size = get_record_size();
    let mut groups: HashMap<i64, Vec<u8>> = HashMap::new();
    for i in 0..count {
        let rec = &p.records[i * rec_size..(i + 1) * rec_size];
        let key: i64 = unsafe { fetch_ptr(rec.as_ptr().add(key_off)) };
        groups.entry(key).or_default().extend_from_slice(rec);
    }
    let row_size = FIELD_SIZE + agg.step_total; // key + fields
    let mut out = vec![0_u8; groups.len() * row_size];
    let mut work = vec![0_u8; KEY_BUF_SIZE]; // per-key aggregate work area (state included)
    let work_ptr = work.as_mut_ptr();
    let mut off = 0;
    for (key, recs) in groups.iter() {
        let rec_count = recs.len() / rec_size;
        unsafe { *(out.as_mut_ptr().add(off) as *mut i64) = *key };
        let row_out = unsafe { out.as_mut_ptr().add(off + FIELD_SIZE) };
        match &agg.agg_jit {
            Some(jit) => {
                unsafe { jit.call(recs.as_ptr(), rec_count, rec_size, work_ptr) };
            }
            None => {
                if !init_data(work_ptr, &agg.wrapped) {
                    return;
                }
                if rec_count > 0 {
                    let param = CallbackParams::new(recs.as_ptr(), 0, 0, rec_count, rec_size);
                    compute_data(work_ptr, &agg.wrapped, stream, param, &agg.stream_offsets);
                }
            }
        }
        // copy the dense field results into the output row
        unsafe {
            std::ptr::copy_nonoverlapping(work_ptr, row_out, agg.step_total);
        }
        off += row_size;
    }
    let param = CallbackParams::new_with_window(
        out.as_ptr(),
        0,
        0,
        groups.len(),
        row_size,
        p.start_ms,
        p.end_ms,
    );
    callback(agg.wrapped.fn_holder(), param);
}

/// Starts the timer thread if it is not already running.
/// Unlike `Once`, the timer can be restarted after `stop_timer()` (tests and
/// embedding scenarios stop and restart the engine within one process).
fn start_timer() {
    let mut handle = TIMER_HANDLE.lock().unwrap_or_else(|e| e.into_inner());
    if TIMER_RUNNING.load(Ordering::SeqCst) {
        return;
    }
    TIMER_RUNNING.store(true, Ordering::SeqCst);
    *handle = thread::Builder::new()
        .name("bpe-window-timer".to_string())
        .spawn(|| {
            while TIMER_RUNNING.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(TICK_MS));
                check_and_trigger();
            }
        })
        .ok();
}

/// Stops the timer thread (called from `stop()`). The thread exits after its
/// current tick; a later `def_window_aggregate` restarts it.
pub(crate) fn stop_timer() {
    TIMER_RUNNING.store(false, Ordering::SeqCst);
}
