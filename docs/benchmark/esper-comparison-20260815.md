# BPE vs Esper 性能对标（特化内核 vs 通用平台）

> 日期：2026-08-15
> 目标：验证 BPE 在**特化场景**（单表规则评估内核）下性能优于 Esper。
> **重要边界**：这是"特化内核 vs 通用 CEP 平台"的对比，仅覆盖三个核心规则评估
> 场景的性能。BPE 的功能完备性远不及 Esper，不构成"全面超越"的结论。

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

## 结论（特化场景）

**同等规则、同等数据、同等测量方式下，BPE 在"单表规则评估内核"这一特化场景
超过 Esper：**

1. **纯过滤 1.8 倍**：JIT 编译 WHERE（LLVM 原生码）vs EPL 解释求值
2. **过滤 + 计算 4.5 倍**：EvalPlan 直读 + 内联算术 vs Esper 表达式解释 + 事件对象转换
3. **窗口聚合 2.2 倍**：JIT 聚合内核（单循环 + phi）vs Esper 增量状态维护

优势根源：无 GC/零分配热路径、全链路 JIT、无序列化（嵌入式直接传指针）、无锁单线程确定性延迟。
**这些优势来自特化**：BPE 只做单表过滤/计算/窗口聚合，把全部优化预算投入这条窄路径；
Esper 要同时支撑通用 CEP 的全部能力，必然承担通用性的解释与调度开销。

## 定位边界（务必说明）

**BPE 的功能完备性远不及 Esper，性能优势仅在特化场景成立：**

| 能力 | BPE | Esper |
|---|---|---|
| 单表过滤/计算/窗口聚合（内核性能） | ✅ 最优（本报告） | 可用 |
| CEP 模式匹配（A→B 序列、超时、缺失事件） | ❌ 无 | ✅ 完整 EPL |
| 多流关联（JOIN/多事件类型） | ❌ 仅单表 | ✅ 完整 |
| 状态持久化 / 故障恢复 / exactly-once | ❌ 进程级 | ✅ |
| watermark / 乱序精确处理 | ⚠️ 仅 lag 容忍 | ✅ 完整 |
| 多线程 / 分布式水平扩展 | ❌ 单线程内核（多实例分区） | ✅ |
| 网络接入（Kafka/消息源） | ❌ 需宿主提供 | ✅ 内置 |
| 运维 / 监控 / 管理接口 | ❌ 日志为主 | ✅ JMX/管理 API |
| 语言生态 | Rust/Java/Python 绑定 | Java（EPL 标准） |

**一句话定位**：
- **Esper**：通用 CEP 平台——什么都能做，全面但有通用性开销。
- **BPE**：特化规则评估内核——只做单表过滤/计算/窗口聚合，用特化换取
  1.8-4.5 倍性能；超出这个范围的能力需要宿主或外部系统补齐。
- 因此本对比的结论是"**在特化场景下 BPE 更快**"，而非"BPE 优于 Esper"。
  选型时：规则复杂/需状态与模式匹配 → Esper/Flink；规则简单高频/嵌入式预筛
  → BPE（或两者级联：BPE 预筛 + Esper 复杂处理）。

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
