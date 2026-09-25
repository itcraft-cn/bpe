# Bamboo Pipe Engine (BPE)

用 Rust 编写的高性能、可嵌入的流式规则引擎：基于 SQL 的查询接口 + LLVM JIT 编译

BPE 面向**风控与告警场景**：高吞吐预过滤、时间窗口聚合、per-key 分组、维表关联——热路径
延迟纳秒级。

## 特性总览

| 能力 | 说明 |
|---|---|
| JIT 过滤 | `WHERE` 条件经 LLVM-19 编译为原生机器码 |
| 多规则 | 同一记录可注册多个 mapper，逐条评估 |
| 标量函数 | 15+ 个（`_abs` `_pow` `_sqrt` `_greatest` `_to_double` ...），SELECT 与 WHERE 均可用 |
| 聚合函数 | `_suml/_count/_avg/_minl/_maxl/_firstl/_lastl`（含 Double 变体）、`_stddev/_variance`（总体/样本） |
| 时间窗口 | 固定窗口（Tumbling）与滑动窗口（Sliding），事件时间或处理时间 |
| Watermark | 事件时间乱序容忍（`lag_ms`），数据驱动提前触发 + wall-clock 兜底 |
| Per-key 分组 | `def_keyed_window_aggregate`：窗口结果按 key 列分组 |
| 维表关联 | 黑名单/限额等 keyed 查询表，SQL `_dim_has(dim_id, key)` / `_dim_get(dim_id, key)` |
| 低开销 | 无锁单线程热路径；无窗口路径零额外开销 |

## 快速开始（Rust）

```toml
[dependencies]
bpe = "<version>"
```

```rust
use bpe::{def_incoming, def_mapper, new_data, start, stop, Column, U8Bytes};

fn main() {
    // 配置文件根目录（cfg/config.toml 位于 BPE_HOME 下）
    std::env::set_var("BPE_HOME", std::env::current_dir().unwrap());
    start();

    // 1. 定义记录模式（Long/Double 列，每列 8 字节）
    let txn = def_incoming("txn", vec![
        Column::new_long("ts"),      // 事件时间（毫秒）
        Column::new_long("user_id"),
        Column::new_long("amount"),
    ]).unwrap();

    // 2. 高速阈值规则：WHERE 经 JIT 编译，每条记录评估
    def_mapper(
        "SELECT txn.user_id, _to_double(txn.amount) FROM txn WHERE txn.amount > 100000 LIMIT 10",
        |p| {
            // p.size() 为命中数，字段密集排列于 p.u8_ptr()
            eprintln!("大额交易告警：{} 条命中", p.size());
        },
    );

    // 3. 发送数据（定长 512 字节记录，列偏移 8 字节对齐）
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

## 时间窗口（风控/告警场景）

```rust
use bpe::{def_window_aggregate, def_keyed_window_aggregate, Window};

// 固定窗口：每 1 分钟上报该分钟交易笔数与金额
def_window_aggregate(
    "SELECT _count(txn.amount), _suml(txn.amount) FROM txn WHERE txn.amount > 0",
    Window::Tumbling { period_ms: 60_000 },
    Some("ts"), 0,                      // 事件时间取 "ts" 列
    |p| {
        let start = p.window_start_ms();
        let end = p.window_end_ms();
        // 聚合结果位于 p.u8_ptr()：[count i64][sum i64]
    },
);

// 滑动窗口：每 60 秒回看近 5 分钟的异常标记次数
def_window_aggregate(
    "SELECT _count(txn.risk) FROM txn WHERE txn.risk = 1",
    Window::Sliding { length_ms: 300_000, slide_ms: 60_000 },
    Some("ts"), 0,
    |p| { /* 窗口 [start, end) 命中数 */ },
);

// per-key：每个用户在每个窗口内的笔数（行布局 [key][field0]...[fieldN]）
def_keyed_window_aggregate(
    "SELECT _count(txn.amount) FROM txn WHERE txn.amount > 0",
    Window::Tumbling { period_ms: 60_000 },
    Some("ts"), "user_id", 0,
    |p| {
        for row in 0..p.size() {
            let r = unsafe { p.u8_ptr().add(row * p.step()) };
            // r 处为 key(i64)，后续为聚合字段
        }
    },
);
```

语义说明：

- **事件时间**：`ts_field` 指定 Long（毫秒）列；窗口按事件时间对齐；`lag_ms` 为乱序容忍，
  早于 watermark（`max_seen_ts - lag_ms`）的记录被丢弃。
- **处理时间**：`ts_field = None`，窗口按到达时间对齐。
- **触发**：窗口满足 `end < watermark`（事件时间推进）或 `end + lag_ms <= now`
  （wall-clock 兜底，数据停止时窗口仍上报）即触发。
- **空窗口也会触发**（`size()==0`，返回聚合初值）。
- 回调在引擎定时器线程执行，需线程安全（Send）且不宜阻塞。

## 维表关联

```rust
use bpe::{def_dimension, update_dimension, def_mapper};

let blacklist = def_dimension().unwrap();
update_dimension(blacklist, 42, 1);          // key -> value

def_mapper(
    "SELECT txn.amount FROM txn WHERE _dim_has(0, txn.user_id) > 0 LIMIT 10",
    |p| { /* 仅维表 0 中存在的用户通过 */ },
);

def_mapper(
    "SELECT _dim_get(0, txn.user_id), txn.amount FROM txn WHERE txn.amount > 0 LIMIT 10",
    |p| { /* 首字段为维表值（不存在为 0） */ },
);
```

## SQL 函数参考

函数以 `_` 前缀调用：`_name(args)`。

### 标量（SELECT 字段与 WHERE 条件均可用）

| 函数 | 说明 | | 函数 | 说明 |
|---|---|---|---|---|
| `_add/_sub/_mul/_div/_mod(a,b)` | 算术 | | `_sign(x)` | 符号（-1/0/1） |
| `_abs(x)` | 绝对值 | | `_trunc(x)` | 截断小数 |
| `_ceil/_floor/_round(x)` | 取整 | | `_to_long(x)` | 转 i64 |
| `_sqrt(x)` | 平方根 → f64 | | `_to_double(x)` | 转 f64 |
| `_exp(x)` | e^x → f64 | | `_pow(a,b)` | 幂 → f64 |
| `_ln(x)` | 自然对数 → f64 | | `_greatest/_least(a,b)` | 取大/取小 |
| `_log10(x)` | 常用对数 → f64 | | `_dim_has/_dim_get(dim_id,key)` | 维表查询 |

### 聚合（窗口 / 聚合 SQL）

| 函数 | 说明 | | 函数 | 说明 |
|---|---|---|---|---|
| `_suml/_sumd(x)` | 求和 | | `_avg(x)` | 均值（f64） |
| `_count(x)` | 计数（任意列类型） | | `_firstl/_firstd(x)` | 首值 |
| `_minl/_minl(x)` / `_maxl/_maxd(x)` | 最小/最大 | | `_lastl/_lastd(x)` | 末值 |
| `_stddev(x)` | 总体标准差（f64） | | `_stddev_samp(x)` | 样本标准差（f64） |
| `_variance(x)` | 总体方差（f64） | | `_var_samp(x)` | 样本方差（f64） |

## 配置

`cfg/config.toml`（根目录由 `BPE_HOME` 环境变量指定）：

```toml
dev_mode = true      # true: 控制台+文件日志；false: 仅文件日志
vec_size = 1048576   # 环形缓冲字节数（须为 2 的幂）
record_size = 512    # 单条记录槽字节数（最大 8192；须 ≥ 列数×8）
log_dir = "/tmp"     # 日志目录
```

## 构建

```bash
RUSTFLAGS="-lLLVM-19" cargo build --release
```

依赖：Rust 2021、LLVM-19

## 性能

criterion 全链路测量（`new_data → 入环形缓冲 → 窗口扫描 → JIT 过滤 → 字段提取 → 回调`），release 构建：

| 场景 | 中位数 | 单核吞吐（约） |
|---|---|---|
| new_data + 过滤（4 字段，LIMIT 10） | ~0.44 µs/op | ~230 万条/秒 |
| new_data + 过滤 + 聚合（JIT 内核） | ~0.46 µs/op | ~220 万条/秒 |

### 全链路分解（perf_diff：6 条件 WHERE + 5 计算字段 SELECT，窗口满）

| 阶段 | 成本 | 占比 |
|---|---|---|
| insert（512B memcpy） | ~14 ns | 4.5% |
| 过滤 + 位图扫描 | ~106 ns | 34% |
| 字段提取 + 回调（10 条 × 5 字段） | ~194 ns | 62% |
| **合计** | **~314 ns/op** | — |

纯聚合路径（bind `def_aggregate`，5 字段 × LIMIT 10）：~78 ns/op（~1300 万条/秒）。

### 优化历程（perf_diff 场景，8.4 µs/op → 0.31 µs/op，约 **27 倍**）

1. **增量命中位图**：JIT 过滤在插入时每条记录仅执行一次（而非每次窗口扫描），结果按 slot 1 bit 存储
2. **64 位块扫描**：跳过零字 + TZCNT/LZCNT 定位置位（SIMD 友好的位运算）
3. **预解析字段读取计划**：SELECT 字段编译为 (offset, type) 直读 / 扁平化表达式树
4. **聚合 JIT 内核**：整批聚合编译为单个 LLVM 循环（直读 + 内联算术 + phi 累加器）——绑定/def_aggregate 路径从 ~82µs/op 降至 ~78ns/op（超 1000 倍）

热路径无锁、Rust 侧零分配：连续环形缓冲内存、预计算列偏移、JIT 编译过滤、
回调缓冲复用。无窗口路径不触碰窗口锁，纯过滤性能不受窗口功能影响。

### 本机复测记录（2026-09-25）

直接运行 `examples/` 复测（非 criterion）；release 构建，环境 Intel Core Ultra 9 285H
（16 线程，单 NUMA，实测 CPU scaling 约 49%）；单消费者线程；各示例重复 3 次，波动约 ±3~5%。

| 示例 | 场景 | ns/op | 约 events/s |
|---|---|---:|---:|
| `perf_rsize` | 入站（512B / 64B 记录） | 147.9 / 147.6 | ~680 万 |
| `perf_aligned` | 过滤 6 条件 + 1 字段 | ~149 | ~670 万 |
| `perf_aligned2` | 过滤 1 条件 + 5 计算字段（LIMIT 1） | 50.0 | ~2000 万 |
| `perf_agg` | 过滤 + 聚合（bind 路径，解释执行） | 68.1 | ~1470 万 |
| `perf_diff` | 入站 + 过滤 + fetch + 回调 | 368~399 | ~250~270 万 |
| `sample` | 入站 → mapper（过滤）→ aggregate，逐事件回调 | 983 | ~100 万 |

`perf_diff` 分解（复测）：insert ~15.8 ns；filter+scan ~118~121 ns；fetch+callback ~236~260 ns。

要点：

- **debug 与 release 差距显著**：`sample` debug 约 3460 ns/op，release 约 983 ns/op（约 3.5×）。性能须以 release 为准。
- **瓶颈在管道投递，不在 SQL/JIT 求值**：`fetch+callback`（队列投递 + 回调派发）为最大单项；
  6 条件过滤仅 ~118 ns，聚合内核 ~68 ns。
- **记录大小无影响**：64B 与 512B 均为 ~148 ns/op，此规模下由每记录固定开销主导。
- 以上数字与上文 criterion / Esper 小节存在小幅差异（机器、测量方式、版本不同），此处仅作记录。

### 对标 Esper（特化内核基准，同规则同数据，单核）

| 场景 | BPE | Esper 7.1 | 优势 |
|---|---|---|---|
| 纯过滤（6 条件） | 133 ns | 240 ns | **1.8×** |
| 过滤 + 5 计算字段 | 49 ns | 222 ns | **4.5×** |
| 窗口聚合（len 10） | 78 ns | 172 ns | **2.2×** |

> **边界声明**：这是**特化单表规则评估内核**与**通用 CEP 平台**的对比，优势源于
> 特化——BPE 只做单表过滤/计算/窗口聚合；Esper 完整支持 CEP 模式匹配、多流
> 关联、状态持久化、exactly-once、watermark、分布式、内置接入、JMX 运维等。
> **BPE 不是 Esper 的通用替代**：复杂规则/有状态模式用 Esper/Flink；
> 高频嵌入式预筛用 BPE（或级联 BPE 预筛 → Esper 复杂处理）。
> 方法学与复现见 `docs/benchmark/esper-comparison-20260815.md`。

## 测试

```bash
RUSTFLAGS='-lLLVM-19' cargo test
```

42 个测试二进制：单元测试、SQL 解析、过滤/聚合正确性、窗口语义（固定/滑动/
处理时间/watermark）、per-key 分组、维表查询，以及带断言的回归测试。

## 文档索引

- `docs/design.md` — 架构说明
- `docs/deconstruct/` — 设计解构（类图/数据流图、算法、内存分析）
- `docs/review/` — 代码审查报告
- `docs/detect/` — 问题侦测与评分
- `docs/refactor/` — 重构方案与执行状态
- `docs/code-part-modification/task-*` — 各任务变更记录

## 依赖

- LLVM 19（经 inkwell 的 JIT）、sql-parse（SQL 方言）、log4rs（日志）、
  hashbrown、core_affinity、strum、config
