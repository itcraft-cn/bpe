# Bamboo Pipe Engine (BPE)

A high-performance, embeddable streaming rule engine written in Rust, with SQL-based
queries, LLVM JIT compilation, and multi-language bindings (Rust / Java / Python).

BPE targets **risk-control and alerting workloads**: high-throughput pre-filtering,
time-window aggregation, per-key grouping, and keyed dimension lookups — all with
nanosecond-level hot-path latency.

## Feature Overview

| Capability | Description |
|---|---|
| JIT filtering | `WHERE` clauses compiled to native machine code via LLVM-19 |
| Multiple rules | Multiple mappers per record, all evaluated per record |
| Scalar functions | 15+ functions (`_abs`, `_pow`, `_sqrt`, `_greatest`, `_to_double`, ...) usable in both `SELECT` and `WHERE` |
| Aggregate functions | `_suml/_count/_avg/_minl/_maxl/_firstl/_lastl` (+Double variants), `_stddev/_variance` (population & sample) |
| Time windows | Fixed (`Tumbling`) and sliding (`Sliding`) windows, event-time or processing-time |
| Watermark | Event-time out-of-order tolerance (`lag_ms`), data-driven early firing + wall-clock fallback |
| Per-key grouping | `def_keyed_window_aggregate`: window results grouped by a key column |
| Dimension tables | Keyed lookup tables (blacklists, quotas) via `_dim_has(dim_id, key)` / `_dim_get(dim_id, key)` |
| Multi-language | Rust (native), Java (JNI/bpe4j), Python (PyO3/bpe4py) |
| Low overhead | Lock-free single-thread hot path; window-less path has zero added cost |

## Quick Start (Rust)

```toml
[dependencies]
bpe = { path = "bpe-core" }
```

```rust
use bpe::{def_incoming, def_mapper, new_data, start, stop, Column, U8Bytes};

fn main() {
    // the config root (cfg/config.toml lives under BPE_HOME)
    std::env::set_var("BPE_HOME", std::env::current_dir().unwrap());
    start();

    // 1. define the record schema (Long/Double columns, 8 bytes each)
    let txn = def_incoming("txn", vec![
        Column::new_long("ts"),      // event time (ms)
        Column::new_long("user_id"),
        Column::new_long("amount"),
    ]).unwrap();

    // 2. high-speed threshold rule: JIT-compiled WHERE, called per record
    def_mapper(
        "SELECT txn.user_id, _to_double(txn.amount) FROM txn WHERE txn.amount > 100000 LIMIT 10",
        |p| {
            // p.size() matched records, dense fields at p.u8_ptr()
            eprintln!("large transaction alert: {} matches", p.size());
        },
    );

    // 3. send data (payload is a fixed 512-byte record; columns at 8-byte offsets)
    let mut payload = vec![0_u8; 512];
    unsafe {
        *(payload.as_mut_ptr() as *mut i64) = 1_700_000_000_000_i64;        // ts
        *(payload.as_mut_ptr().add(8) as *mut i64) = 42_i64;                // user_id
        *(payload.as_mut_ptr().add(16) as *mut i64) = 500_000_i64;          // amount
    }
    new_data(&U8Bytes::new_from_vec(txn, 512, payload));

    stop();
}
```

## Time Windows (risk-control style)

```rust
use bpe::{def_window_aggregate, def_keyed_window_aggregate, Window};

// Fixed window: report every 1 minute the count/sum of transactions
def_window_aggregate(
    "SELECT _count(txn.amount), _suml(txn.amount) FROM txn WHERE txn.amount > 0",
    Window::Tumbling { period_ms: 60_000 },
    Some("ts"), 0,                      // event time from column "ts"
    |p| {
        let start = p.window_start_ms();
        let end = p.window_end_ms();
        // aggregate result at p.u8_ptr(): [count i64][sum i64]
    },
);

// Sliding window: every 60s, look back 5 minutes for abnormal-flag count
def_window_aggregate(
    "SELECT _count(txn.risk) FROM txn WHERE txn.risk = 1",
    Window::Sliding { length_ms: 300_000, slide_ms: 60_000 },
    Some("ts"), 0,
    |p| { /* window [start, end) risk count */ },
);

// Per-key: per-user counts within each window (rows [key][field0]...[fieldN])
def_keyed_window_aggregate(
    "SELECT _count(txn.amount) FROM txn WHERE txn.amount > 0",
    Window::Tumbling { period_ms: 60_000 },
    Some("ts"), "user_id", 0,
    |p| {
        for row in 0..p.size() {
            let r = unsafe { p.u8_ptr().add(row * p.step()) };
            // key: i64 at r, fields follow
        }
    },
);
```

Semantics:
- **Event time**: `ts_field` names a Long (ms) column. Windows align to event time;
  `lag_ms` is the out-of-order tolerance — a record older than the watermark
  (`max_seen_ts - lag_ms`) is dropped.
- **Processing time**: pass `ts_field = None`; windows align to arrival time.
- **Firing**: a window fires when `end < watermark` (event time advances) or
  `end + lag_ms <= now` (wall-clock fallback, so idle windows still report).
- **Empty windows** still fire with `size()==0` (initial aggregate values).
- Callbacks run on the engine's timer thread; they must be `Send` and fast.

## Dimension Tables

```rust
use bpe::{def_dimension, update_dimension, def_mapper};

let blacklist = def_dimension().unwrap();
update_dimension(blacklist, 42, 1);          // key -> value

def_mapper(
    "SELECT txn.amount FROM txn WHERE _dim_has(0, txn.user_id) > 0 LIMIT 10",
    |p| { /* only users present in dimension 0 pass */ },
);

def_mapper(
    "SELECT _dim_get(0, txn.user_id), txn.amount FROM txn WHERE txn.amount > 0 LIMIT 10",
    |p| { /* first field = dimension value (0 if absent) */ },
);
```

## SQL Function Reference

Functions are invoked with a leading underscore: `_name(args)`.

### Scalar (usable in `SELECT` fields and `WHERE` conditions)

| Function | Description | | Function | Description |
|---|---|---|---|---|
| `_add/_sub/_mul/_div/_mod(a,b)` | arithmetic | | `_sign(x)` | sign (-1/0/1) |
| `_abs(x)` | absolute value | | `_trunc(x)` | truncate fraction |
| `_ceil/_floor/_round(x)` | rounding | | `_to_long(x)` | cast to i64 |
| `_sqrt(x)` | square root → f64 | | `_to_double(x)` | cast to f64 |
| `_exp(x)` | e^x → f64 | | `_pow(a,b)` | power → f64 |
| `_ln(x)` | natural log → f64 | | `_greatest/_least(a,b)` | max/min |
| `_log10(x)` | base-10 log → f64 | | `_dim_has/_dim_get(dim_id,key)` | dimension lookup |

### Aggregate (window / aggregate SQL)

| Function | Description | | Function | Description |
|---|---|---|---|---|
| `_suml/_sumd(x)` | sum | | `_avg(x)` | mean (f64) |
| `_count(x)` | count (any column type) | | `_firstl/_firstd(x)` | first value |
| `_minl/_minl(x)` / `_maxl/_maxd(x)` | min/max | | `_lastl/_lastd(x)` | last value |
| `_stddev(x)` | population stddev (f64) | | `_stddev_samp(x)` | sample stddev (f64) |
| `_variance(x)` | population variance (f64) | | `_var_samp(x)` | sample variance (f64) |

## Multi-language Bindings

### Python (bpe4py, PyO3)

```python
import bpe4py as bpe

bpe.start()
txn = bpe.def_stream("txn", ["ts", "user_id", "amount"], [0, 0, 0], [8, 8, 8])

def on_window(data, start_ms, end_ms):
    # data: bytes of the aggregate result; start_ms/end_ms: window range
    print("window", start_ms, end_ms, "bytes:", len(data))

bpe.def_window_aggregate(
    "SELECT _count(txn.amount) FROM txn WHERE txn.amount > 0",
    window_type=1, period_ms=60000, length_ms=0, slide_ms=0,
    ts_field="ts", lag_ms=0, callback=on_window,
)
# bpe.def_keyed_window_aggregate(...), bpe.def_dimension()/update_dimension/remove_dimension
```

> Building the Python wrapper requires PyO3 0.20 (Python ≤ 3.12): set
> `PYO3_PYTHON=/path/to/python3.11`.

### Java (bpe4j, JNI)

```java
JavaBpe.start();
int txn = JavaBpe.defStream("txn", List.of(
    new ColumnDefine("ts", ColumnDefine.Type.LONG, 8),
    new ColumnDefine("user_id", ColumnDefine.Type.LONG, 8),
    new ColumnDefine("amount", ColumnDefine.Type.LONG, 8)
));
JavaBpe.defWindowAggregate(
    "SELECT _count(txn.amount) FROM txn WHERE txn.amount > 0",
    JavaBpe.WINDOW_TUMBLING, 60_000, 0, 0, "ts", 0,
    (data, size) -> { /* aggregate result bytes */ }
);
int dim = JavaBpe.defDimension();
JavaBpe.updateDimension(dim, 42L, 1L);
```

## Configuration

`cfg/config.toml` (root resolved via `BPE_HOME`):

```toml
dev_mode = true      # true: console + file logs; false: file only
vec_size = 1048576   # circular buffer bytes (must be a power of two)
record_size = 512    # bytes per record slot (max 8192; keep ≥ columns × 8)
log_dir = "/tmp"     # log directory
```

## Building

```bash
./build.sh 0   # debug build (core)
./build.sh 1   # release build (core)
./build.sh 2   # full debug build (core + Java + Python)
./build.sh 3   # full release build (core + Java + Python)
```

Requirements: Rust 2021, LLVM-19, Java 8+ (for bpe4j), Python 3.11/3.12 (for bpe4py).

## Performance

Measured with criterion on the full pipeline
(`new_data → insert → window scan → JIT filter → field fetch → callback`), release build:

| Scenario | Median | ~Throughput (single core) |
|---|---|---|
| new_data + filter (4 fields, LIMIT 10) | ~0.44 µs/op | ~2.3M rec/s |
| new_data + filter + aggregate (JIT kernel) | ~0.46 µs/op | ~2.2M rec/s |

### Full-path breakdown (perf_diff: 6-condition WHERE + 5 computed SELECT fields, window full)

| Stage | Cost | Share |
|---|---|---|
| insert (512 B memcpy) | ~14 ns | 4.5% |
| filter + bitmap scan | ~106 ns | 34% |
| field fetch + callback (10 × 5 fields) | ~194 ns | 62% |
| **total** | **~314 ns/op** | — |

Aggregate-only path (bind `def_aggregate`, 5 fields × LIMIT 10): ~78 ns/op (~13M rec/s).

### Optimization history (perf_diff scenario, 8.4 µs/op → 0.31 µs/op, ≈27×)

1. **Incremental hit bitmap**: the JIT filter runs once per record at insert time
   (not once per window scan); result stored as 1 bit/slot.
2. **64-bit bitmap scanning**: zero words skipped, set bits located via
   TZCNT/LZCNT (SIMD-friendly).
3. **Pre-resolved field readers & evaluation plans**: SELECT fields compile to
   direct (offset, type) reads / flattened expression trees.
4. **JIT aggregate kernels**: the whole batch aggregation compiles into a single
   LLVM loop (direct loads, inlined arithmetic, phi accumulators) — the
   bind/`def_aggregate` path went from ~82 µs/op to ~78 ns/op (>1000×).

The hot path is lock-free and allocation-free on the Rust side: continuous
circular-buffer memory, pre-computed column offsets, JIT-compiled filters, and a
reused callback buffer. The window-less path never takes the window lock, so
plain filtering is unaffected by windowing features.

### vs Esper — specialized-kernel benchmark (same rules & data, single core)

| Scenario | BPE | Esper 7.1 | Advantage |
|---|---|---|---|
| pure filter (6 conditions) | 133 ns | 240 ns | **1.8×** |
| filter + 5 computed fields | 49 ns | 222 ns | **4.5×** |
| window aggregate (len 10) | 78 ns | 172 ns | **2.2×** |

> **Scope caveat**: this compares a **specialized single-table rule-evaluation
> kernel** against a **general-purpose CEP platform**. The advantage comes from
> specialization: BPE only does single-table filter/compute/window aggregation,
> while Esper supports full CEP (pattern matching, multi-stream joins, state
> persistence, exactly-once, watermarks, distribution, built-in I/O, JMX ops).
> BPE is **not** a general replacement — for complex rules/stateful patterns use
> Esper/Flink; for high-frequency embedded pre-filtering use BPE (or cascade
> BPE → Esper). See `docs/benchmark/esper-comparison-20260815.md`.

## Testing

```bash
RUSTFLAGS='-lLLVM-19' cargo test
```

42 test binaries: unit tests, SQL parsing, filter/aggregate correctness, window
semantics (fixed/sliding/processing-time/watermark), per-key grouping, dimension
lookups, and regression tests with assertions.

## Documentation

- `docs/design.md` — architecture notes
- `docs/deconstruct/` — design deconstruction (class/data-flow diagrams, algorithms, memory analysis)
- `docs/review/` — code review reports
- `docs/detect/` — problem detection & scoring
- `docs/refactor/` — refactoring plan and status
- `docs/code-part-modification/task-*` — per-task change logs

## Dependencies

- LLVM 19 (JIT via inkwell), sql-parse (SQL dialect), log4rs (logging),
  hashbrown, core_affinity, strum, config, PyO3 / JNI (bindings).
