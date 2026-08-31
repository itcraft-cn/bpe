# 性能优化路线（下一阶段 TODO）

- **日期**：2026-08-31
- **状态**：规划，未排期
- **前置事实**：README 中 0.31 µs/op、2.3M rec/s、Esper 1.8-4.5× 均为**同步回调时代**数字；egress 异步化（ebd2344、088ac26）后热路径新增 channel send + 每事件 Box 分配，全路径数字待重测。**任何进一步优化的决策必须以重测基线为前提**。

## 优化点 A：Java Unsafe 直读（极端性能选项）

### 定位

消除回调链路上的 JNI `conv_array` 整块拷贝与 `byte[]` 分配（当前 JavaFfiFunc 每回调一次拷贝 + 一次 JNI call_method，约 1-5 µs/次）。适用场景：极端低延迟订阅、高频小结果集。属可选增强，非必做项。

### 架构形态

数据不动、通知异步（Aeron/Disruptor 成熟路线）：

```text
Rust 环形缓冲（数据已在槽位，从不搬运）
        │  序号发布（release 语义）
        ▼
Java Unsafe.getLongVolatile(acquire) 读序号 ──► 窗口内直读槽位 ──► 本地解析
回调退化为纯通知信号（不再携带 payload 拷贝）
```

### 四条红线（违反皆为静默错值，非显式报错）

| # | 红线 | 要点 | 事故形态 |
| --- | --- | --- | --- |
| 1 | 端序一致 | `Unsafe.getLong` 为 native 小端与 Rust 一致；`ByteBuffer` 默认 BIG_ENDIAN 必须显式 `LITTLE_ENDIAN` | 数值读反 |
| 2 | 可见性 | Java 侧 acquire 读序号（getLongVolatile / VarHandle getAcquire），序号栅栏通过后数据体方可普通读 | 偶发半新半旧值 |
| 3 | 槽位生命周期 | 先拷出再解析或严格序号窗口内；未发布槽位是未初始化内存 | 读到垃圾/被覆盖数据 |
| 4 | 分配对齐 | Rust 侧 `Layout::from_size_align(vec_size, 1)` 契约仅 1 字节对齐；ARM64 非对齐读有陷阱，应将对齐提至 8/64 | 性能塌陷或平台差异 |

### 机制化保障（先于生产代码就位）

1. **Canary 握手**：引擎 start 时 Rust 写入已知样本（i64 魔数 + 已知 double + 已知布局），Java 直读比对，不一致 fail-fast 拒绝启动
2. **布局单一事实源**：新增 `fieldOffsets(recordId)` 查询 API，Java 不硬编码偏移，消除双端 schema 漂移
3. **双端 golden 测试进 CI**：Rust 写固定样本 → Java Unsafe 解析 → 逐字节比对

### 分阶段

- P0：canary 握手 + fieldOffsets API + golden 测试（机制先行，约 2-3 天）
- P1：只读直读（结果集读取走 Unsafe，提交与回调形态不变）
- P2：通知化回调（回调只带槽位序号，payload 零拷贝）

### 收益边界

只省回调链路拷贝与 GC 压力，**不改变热路径本身**；全路径收益取决于回调占比（fetch+callback 段占 62%，理论上限明显，但需实测）。

## 优化点 B：Rust 侧进一步优化（可选，难度高）

### Step-0：JIT host features 验证（半天，免费捡钱）

`jit/base.rs` 的 `create_jit_execution_engine(OptimizationLevel::Aggressive)` 未显式设置 TargetMachine——JIT 代码基线可能仅 SSE2（generic CPU）。验证方法：dump JIT 产物汇编，检查是否出现 ymm/vex 编码。若未开启 host features，仅加配置即可获得标量与向量化改进。

### Step-1：批式 WHERE 上界测试（半天）

人为攒 16 条记录跑现有 JIT filter，对比 16 次单条调用，取得批处理收益上界数字，再决定是否投入批内核。

### Step-2：批式 WHERE JIT 内核（视 Step-1 结果）

引擎核排空 ingress 队列时天然成批（8-32 条/批）；批内核 4×i64 向量化比较 + movemask 直写 hitmap。一次实现，x86 产 VPCMPEQ、ARM64 走 LLVM 自动向量化，双平台等阶。

### 天花板分析（既有结论，重申）

- insert 4.5% / filter+bitmap 34% / fetch+callback 62%：瓶颈为跨 512B 步长访存与回调，非计算
- SIMD 于现有架构全路径上限约 1.2-1.3×；批内核对 filter 段 2-3×
- 更大杠杆为列式影子列（中改）与 per-key 分片（低破坏、N× 线性扩展）——分片优先级高于 SIMD

## TODO 清单

- [ ] **异步路径性能重测**（最高优先，一切优化决策的前提）
- [ ] Step-0：dump JIT 汇编验证 host features
- [ ] Step-1：批式 WHERE 收益上界测试
- [ ] A-P0：canary 握手机制 + fieldOffsets API + golden 测试
- [ ] A-P1：只读直读（视 P0 与重测结果）
- [ ] A-P2：通知化回调（视 P1）
- [ ] Step-2：批式 WHERE 内核（视 Step-1）
- [ ] （更远）列式影子列 / per-key 分片评估
