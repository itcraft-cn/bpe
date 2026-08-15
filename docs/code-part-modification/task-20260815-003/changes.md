# 修改与验证记录

> task-20260815-003：时间窗口支持（滑动窗口 / 固定窗口 / 时间周期）

## 修改清单

| # | 文件 | 修改内容 |
|---|------|----------|
| C1 | param.rs | CallbackParams 新增 `win_start_ms/win_end_ms` 字段 + `new_with_window` + `window_start_ms()/window_end_ms()` getter（API 向后兼容） |
| C2 | aggregate.rs | 提升 `init_data/compute_data/gen_aggregate/WrappedAggregate::new/fn_holder` 为 pub(crate)，供窗口模块复用聚合内核 |
| C3 | window.rs | 新模块：Window 枚举、桶管理、JIT 过滤落桶、定时器线程（20ms tick，可重启）、到期触发（锁外计算） |
| C4 | id.rs | next_window_id 独立计数器 |
| C5 | core.rs | `def_window_aggregate` 公共 API；new_data 集成 on_new_data（无窗口注册时零开销）；stop 集成 stop_timer |
| C6 | lib.rs | 导出 `Window` / `def_window_aggregate` |
| C7 | data.rs | U8Bytes payload 改 `#[repr(align(8))]` AlignedBytes——修复 JIT 过滤/列读取对未对齐指针的 UB（窗口路径直接读 U8Bytes 暴露了既有隐患） |
| C8 | bpe-test/tests/test_022_window.rs | 5 项窗口断言测试 |

## 新增 API

```rust
pub enum Window {
    None,
    Tumbling { period_ms: u64 },              // 固定窗口
    Sliding { length_ms: u64, slide_ms: u64 }, // 滑动窗口
}

pub fn def_window_aggregate<F>(
    sql: &str,            // "SELECT _count(s.a), _suml(s.a) FROM s WHERE s.a > 100"
    window: Window,       // 窗口配置
    ts_field: Option<&str>, // 事件时间列（Long 毫秒）；None = 处理时间
    lag_ms: u64,          // 迟到容忍
    func: F,              // 窗口到期回调
) -> Option<u16>
```

**语义**：
- 窗口聚合监听 SQL `FROM` 指定的 record；`new_data` 直接用该 record id 发数据
- 记录先过 JIT WHERE 过滤，命中则按事件时间落入覆盖的窗口桶（滑动窗口可多桶）
- 窗口 `end_ms + lag_ms` 到期（定时器线程检测）→ 对桶内记录聚合 → 回调
- 空窗口也触发（size=0，初始值）——"每周期无异常也要报告"
- 回调参数：`u8_ptr` 指向聚合结果（布局同 def_aggregate），`window_start_ms()/end_ms()` 携带窗口时间，`size()` = 1/0

## 验证记录

| 步骤 | 结果 |
|------|------|
| 编译（debug） | ✅ 零错误零警告 |
| test_022_window（5 项） | ✅ 全过：固定窗口事件时间、固定窗口处理时间、滑动窗口多桶归属、过滤+空窗口、多窗口实例 |
| 全量测试 | ✅ 41 个测试二进制全部通过（新增 5 窗口测试，零回归） |
| 性能基准 | ✅ filter 635ns / filter+agg 1411ns，与基线零回退（无窗口路径 fast-path 零开销） |

## 过程中发现并修复的隐患

1. **U8Bytes 未对齐 UB**（C7）：内嵌 `[u8; 8192]` 对齐 1，窗口路径直接对其调用 JIT 过滤与 i64 列读取 → debug 下 misaligned panic。修复：payload 8 字节对齐。（既有测试用栈数组写 i64 亦存在同类风险，已在新测试中用 Vec 构造规避）
2. **定时器不可重启**：start_timer 用 Once，stop() 后无法重启（多测试/嵌入场景）。改为 RUNNING 标志 + JoinHandle 管理，可重启。
3. **滑动窗口首起点**：起点须满足 `s > ts - length`（半开区间），初版少一个 slide。

## 明确不做（文档化）

- Session 窗口、窗口内明细回调（初版仅聚合）
- 乱序精确处理（watermark）：仅 lag_ms 简单容忍
- 持久化/状态恢复：进程级
- Java/Python 封装暴露窗口 API：后续版本
- 说明：事件时间窗口按事件时间划分（与 wall-clock 无关）；lag_ms 内迟到的记录仍可入桶（桶未触发前）

## 风控/告警使用示例

```rust
// 每 1 分钟报告"该分钟交易笔数与金额"（事件时间）
def_window_aggregate(
    "SELECT _count(txn.amount), _suml(txn.amount) FROM txn WHERE txn.amount > 0",
    Window::Tumbling { period_ms: 60_000 },
    Some("ts"), 0,
    |p| { /* p.window_start_ms()/end_ms() + count/sum */ },
);

// 近 5 分钟、每分钟滑动一次的"异常次数"滑动窗口
def_window_aggregate(
    "SELECT _count(txn.risk) FROM txn WHERE txn.risk = 1",
    Window::Sliding { length_ms: 300_000, slide_ms: 60_000 },
    Some("ts"), 0,
    |p| { /* 每次滑动检查最近 5 分钟命中数 */ },
);
```
