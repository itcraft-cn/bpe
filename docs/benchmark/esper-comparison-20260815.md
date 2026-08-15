# BPE vs Esper 性能对标

> 日期：2026-08-15
> 目标：验证 BPE 在同等场景下性能超过 Esper（CEP 业界经典引擎）

## 环境与方法

| 项 | BPE | Esper |
|---|---|---|
| 版本 | 0.3.0-dev（release 构建） | 7.1.0 |
| 运行 | Rust 单线程（core_affinity 绑核） | JVM 单线程（JDK 25） |
| 事件模型 | 复用 U8Bytes（512B 固定记录） | 复用 Map 事件 |
| 规则 | SQL → LLVM JIT | EPL 解释 |
| 测量 | 预热 5 万条 + 100 万条计时 | 预热 10 万条 + 100 万条计时（3 轮取最优） |
| 回调 | 空回调 | 空监听器 |

数据模型：8 列 Long（ts/user_id/amount/risk/a/b/c/d），同字段、同数据分布。

## 结果（单核，ns/op）

| 场景 | BPE | Esper | BPE 优势 |
|---|---|---|---|
| **纯过滤**（6 条件 WHERE） | **133 ns** | 240 ns | **1.8×** |
| **过滤 + 5 计算字段**（每事件输出 1 行） | **49 ns** | 222 ns | **4.5×** |
| **窗口聚合**（len 10，每事件更新） | **78 ns** | 172 ns | **2.2×** |

（Esper 的 `len_batch(10)` 125ns 为"每 10 条输出一次"，输出频率 1/10，工作量不对等，不列入直接对比。）

## 吞吐（events/s，单核）

| 场景 | BPE | Esper |
|---|---|---|
| 纯过滤 | ~750 万 | ~420 万 |
| 过滤 + 5 计算字段 | ~2030 万 | ~450 万 |
| 窗口聚合 | ~1290 万 | ~580 万 |

## 结论

**同等规则、同等数据、同等测量方式下，BPE 在全部三个核心场景超过 Esper：**

1. **纯过滤 1.8 倍**：JIT 编译 WHERE（LLVM 原生码）vs EPL 解释求值
2. **过滤 + 计算 4.5 倍**：EvalPlan 直读 + 内联算术 vs Esper 表达式解释 + 事件对象转换
3. **窗口聚合 2.2 倍**：JIT 聚合内核（单循环 + phi）vs Esper 增量状态维护

优势根源：无 GC/零分配热路径、全链路 JIT、无序列化（嵌入式直接传指针）、无锁单线程确定性延迟。

## 复现

```bash
# BPE 侧（已含在 bpe-test/src/bin/）
cd /disk2/helly_data/code/rust/bpe
RUSTFLAGS="-lLLVM-19" cargo run --release -p bpe-test --bin perf_aligned   # 纯过滤
RUSTFLAGS="-lLLVM-19" cargo run --release -p bpe-test --bin perf_aligned2 # 过滤+5字段
RUSTFLAGS="-lLLVM-19" cargo run --release -p bpe-test --bin perf_agg      # 窗口聚合

# Esper 侧（docs/benchmark/esper-bench/）
cd docs/benchmark/esper-bench
mvn exec:java -Dexec.mainClass=bench.BenchMain
```

> 注：Esper 结果存在 JVM 波动（GC/JIT 状态），取多轮最优；BPE 无 GC 波动。
> JDK 25 上 Esper 7.1 有 `Math.abs` 反射限制，基准中改用 `amount*2`（与 BPE `_mul(a,2)` 对应）。
