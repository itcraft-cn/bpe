# 计划：per-key 状态 / 维表 / 封装 / watermark

> 日期：2026-08-15（task-20260815-004）
> 目标：补齐风控/告警场景的最后四块能力

## 1. per-key 窗口聚合（GROUP BY key）

```rust
pub fn def_keyed_window_aggregate<F>(
    sql: &str,            // "SELECT _count(s.a), _suml(s.a) FROM s WHERE s.a > 100"
    window: Window,
    ts_field: Option<&str>,
    key_field: &str,      // Long 列（user_id / card_no 等）
    lag_ms: u64,
    func: F,
) -> Option<u16>
```

**回调契约**：窗口到期输出该窗口内**每个 key** 的聚合结果，缓冲布局每行 `[key i64][field0]...[fieldN]`，`size()=key 数`，`step()=(1+字段数)*8`；空窗口 size=0。
**实现**：桶内按 key 分组（HashMap<i64, Vec<u8>>），每组独立工作缓冲（含 stddev 状态区）跑 `init_data/compute_data`。

## 2. 维表关联（keyed lookup）

```rust
pub fn def_dimension() -> Option<u16>          // 分配维表 id
pub fn update_dimension(id: u16, key: i64, value: i64)
pub fn remove_dimension(id: u16, key: i64)
```
SQL 函数（SELECT + WHERE 均可用）：`_dim_has(dim_id, key) -> Long(0/1)`、`_dim_get(dim_id, key) -> Long(value, 缺省 0)`。
**实现**：dimension.rs 模块（RwLock 保护）；注册 JIT 函数 func_dim_has/func_dim_get（binary 签名）；解释器路径 SupportFunc 加 DimHas/DimGet。

## 3. Java/Python 封装

- **Python**（bpe-py-wrapper）：`def_window_aggregate(sql, window_type, period_ms, length_ms, slide_ms, ts_field, lag_ms, callback)` + 维表 API + per-key
- **Java**（bpe4j + bpe-java-wrapper）：Window 编码（int）+ native 方法 + JavaBpe 包装

## 4. watermark 乱序处理（不改 API，增强 lag 语义）

- 事件时间模式：`watermark = max_seen_ts - lag_ms`（数据驱动）
- 触发条件：`end < watermark OR end + lag <= now`（watermark 提前触发 + wall-clock 兜底 idle）
- 迟到丢弃：`end < watermark` 的窗口不入桶（超过乱序容忍的记录丢弃）
- 对现有测试兼容（wall-clock 兜底）

## 实施顺序

watermark（window.rs）→ per-key（window.rs）→ 维表（新模块+JIT+解释器）→ Python → Java → 测试 → 全量回归 + bench
