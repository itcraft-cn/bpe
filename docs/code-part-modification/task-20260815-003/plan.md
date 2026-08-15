# 计划：时间窗口支持（滑动窗口 / 固定窗口 / 时间周期）

> 日期：2026-08-15（task-20260815-003）
> 目标：为风控/告警场景提供时间窗口聚合能力（"1 分钟交易超 5 笔"、"每 5 分钟统计"）

## 需求

1. **固定（滚动）窗口**：`Tumbling { period_ms }`——每 period 毫秒一个窗口，窗口到期触发聚合回调
2. **滑动窗口**：`Sliding { length_ms, slide_ms }`——窗口长 length，每 slide 推进一次，到期窗口触发
3. **时间周期语义**：事件时间（记录携带 timestamp 列，Long 毫秒）或处理时间（到达时间）

## API 设计

```rust
pub enum Window {
    None,                                  // 现有行为：每条记录触发
    Tumbling { period_ms: u64 },           // 固定窗口
    Sliding { length_ms: u64, slide_ms: u64 }, // 滑动窗口
}

pub fn def_window_aggregate<F>(
    sql: &str,            // "SELECT _count(s.a), _suml(s.a) FROM s WHERE s.a > 100"
    window: Window,       // 窗口配置
    ts_field: Option<&str>, // 事件时间列名（Long 毫秒）；None = 处理时间
    lag_ms: u64,          // 迟到容忍（窗口到期后延迟触发）
    func: F,              // 窗口到期回调
) -> Option<u16>
```

回调契约：`CallbackParams` 新增 `window_start_ms()/window_end_ms()`（窗口时间范围），u8_ptr 指向聚合结果（布局同现有聚合：col_idx*8），size=1（有记录）/0（空窗口）。

## 架构

```
new_data ──► 现有路径（insert + mapper 回调，无窗口记录零开销）
        └──► window::on_new_data：锁内 JIT 过滤 → 事件时间定桶 → 记录入桶

定时器线程（TICK=20ms）──► 锁内收集到期桶（end_ms + lag <= now）→ 锁外：
    桶记录 → init_data + compute_data（复用聚合内核）→ 回调(窗口时间 + 结果)
```

### 关键设计

1. **完全复用聚合内核**：窗口触发时用 `init_data/compute_data`（aggregate.rs）对桶内记录计算，回调复用 `FnHolder`；聚合输入 = 过滤后的原始记录（record 布局），`offsets` = stream 列偏移
2. **窗口桶**：`Vec<WindowBucket{start_ms, end_ms, records: Vec<u8>, triggered}>`，记录按 `record_size` 密集存放；滑动窗口一条记录可入多个重叠桶（最多 length/slide 个）
3. **事件时间**：ts_field 列读 i64 毫秒；窗口按事件时间对齐划分；乱序由 lag_ms 容忍
4. **线程安全**：窗口状态（桶/注册表）由 `WINDOW_LOCK: Mutex` 保护；new_data 落桶与定时器触发互斥；聚合计算在锁外执行（各窗口实例独立结果缓冲，无共享竞争）；普通 mapper 路径不碰锁（零开销）
5. **定时器**：首个窗口定义时启动后台线程（daemon），stop() 停止

## 实施顺序

1. param.rs：CallbackParams 加窗口字段 + getter（API 兼容）
2. aggregate.rs：提升 init_data/compute_data/gen_aggregate/WrappedAggregate::new 为 pub(crate)
3. window.rs：新模块（注册表/桶/落桶/定时器/触发）
4. id.rs：next_window_id
5. core.rs：def_window_aggregate + stop 集成
6. lib.rs：导出 Window / def_window_aggregate
7. 测试 test_022_window.rs（固定/滑动/处理时间/过滤/空窗口/多实例）
8. 全量回归 + 性能基准

## 不做（文档化）

- Session 窗口、窗口内明细回调（初版仅聚合）
- 乱序精确处理（watermark 机制）：仅 lag_ms 简单容忍
- 持久化/状态恢复：进程级
- Java/Python 封装暴露窗口 API：后续版本
