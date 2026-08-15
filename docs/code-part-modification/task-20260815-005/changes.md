# 性能优化记录（rust-flamegraph 多轮优化）

> 日期：2026-08-15（task-20260815-005）

## 方法

1. 创建 perf_001 集成测试（200 万条风控热路径：8 列 + 6 条件 WHERE + 5 计算字段 SELECT）
2. cargo flamegraph / perf record + inferno 火焰图（debug 帧指针版 + release 版）
3. perf 差分法（A 无 mapper / C 恒假过滤 / B 全真过滤+计算）量化各阶段成本

## 热点定位（决定性发现）

perf 差分显示瓶颈不是 JIT 过滤本身，而是**每次 new_data 对窗口内全部 2048 条
记录重新执行 JIT 过滤**（O(n²) 架构问题）：

| 阶段 | 成本 | 占比 |
|------|------|------|
| insert（memcpy 512B） | 14 ns | 0.2% |
| filter.call（全窗口 2048 次/记录） | 8119 ns | 96% |
| fetch+callback（10 条 × 5 字段） | 278 ns | 3.3% |

## 四轮优化与结果

| 轮 | 优化 | B 场景 (ns/op) | 累计 |
|----|------|----------------|------|
| 基线 | 全窗口逐条 JIT 过滤 | 8411 | 1× |
| 1 | 增量命中位图（filter 每记录 1 次） | 2806 | 3× |
| 2 | u64 块扫描（跳过零字 + TZCNT/LZCNT） | 359 | 23× |
| 3 | FieldReader 预解析列直读 | 351 | 24× |
| 4 | EvalPlan 表达式求值计划 | 306 | 27.5× |

（修复块扫描 `scanned < count` bug：应比较窗口长度而非总写入数，54µs→114ns，474×）

## 最终指标（release）

| 指标 | 值 |
|------|-----|
| perf_diff B 场景 | 306-314 ns/op（~320 万 ops/s） |
| perf_001（6 条件 + 5 计算字段） | 207 ns/op（483 万 ops/s） |
| criterion filter | 354 ns（原 720） |
| criterion filter+agg | 1166 ns（原 1454） |
| insert 单阶段 | 14 ns |
| 全窗口位图扫描（恒假） | ~100 ns |

## 关键设计

1. **增量位图**：insert 时对每 mapper 执行 1 次 JIT filter → per-slot 1 bit；窗口查询扫位图
2. **块扫描**：64-bit 字跳过零块，`leading_zeros/trailing_zeros`（编译为 LZCNT/TZCNT）定位
3. **维表一致性**：维度版本号 + 引擎线程惰性重建位图（单线程安全，无锁）
4. **FieldReader/EvalPlan**：SELECT 字段定义期编译为直读偏移/扁平表达式树，消除 record 查找与深分派

## 过程中发现并修复的 bug

- 块扫描用总写入数而非窗口长度（导致 781 轮扫描而非 32 轮）
- sed 批量修复时丢失 `add(offset)`（所有字段写到偏移 0）

## 验证

- 全量 44 测试二进制通过（位图语义与逐条过滤一致，含维表动态更新）
- #[inline] 分析后已恢复

## 遗留优化空间（后续）

- EvalPlan::Fallback（聚合表达式）深分派
- 回调参数构造与 FnHolder 分派
- 窗口聚合的 per-key 分组 HashMap 分配
