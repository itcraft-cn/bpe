# 修改与验证记录

> task-20260815-004：per-key 状态 / 维表 / 封装 / watermark

## 修改清单

| # | 文件 | 修改内容 |
|---|------|----------|
| C1 | window.rs | watermark：`max_seen_ts` 追踪 + 触发条件 `end < watermark OR end + lag <= now`；迟到丢弃（`end < watermark` 不入桶）；per-key 分组（`key_offset` + `deliver_keyed`，每 key 独立工作缓冲） |
| C2 | core.rs | `def_keyed_window_aggregate(_ffi)`、`def_window_aggregate_ffi` |
| C3 | lib.rs | 导出 keyed/ffi 变体 + dimension API |
| C4 | dimension.rs | 新模块：维表（RwLock HashMap<i64,i64>），def/update/remove + dim_contains/dim_get |
| C5 | func_enum.rs | DimHas/DimGet 变体 |
| C6 | calc_func.rs | `_dim_has`/`_dim_get` 解释器实现（fetch 2 args → dimension 查询） |
| C7 | exec.rs | compute_func 分派 DimHas/DimGet |
| C8 | jit/aux.rs | jit_dim_has/jit_dim_get（JIT 路径查维表） |
| C9 | jit/base.rs | 注册 func_dim_has/func_dim_get |
| C10 | bpe-py-wrapper | def_window_aggregate / def_keyed_window_aggregate / def_dimension / update_dimension / remove_dimension；PythonWindowFfiFunc（回调传 `(data, start, end)`）；pyo3 signature 注解 |
| C11 | bpe-java-wrapper | JNI：defWindowAggregate / defKeyedWindowAggregate / defDimension / updateDimension / removeDimension；`#![allow(non_snake_case)]` |
| C12 | bpe4j | Bpe native 声明 + JavaBpe 包装（WINDOW_TUMBLING/SLIDING 常量） |
| C13 | Cargo.toml | workspace.exclude = ["bpe-py-wrapper"]（修复独立构建） |
| C14 | test_023_keyed_dimension.rs | 4 项断言测试 |

## 新增 API

```rust
// per-key 窗口聚合
pub fn def_keyed_window_aggregate<F>(sql, window, ts_field, key_field, lag_ms, func) -> Option<u16>
// 回调：每行 [key i64][field0]...[fieldN]，size()=key 数，step()=(1+N)*8

// 维表
pub fn def_dimension() -> Option<u16>
pub fn update_dimension(id, key, value)
pub fn remove_dimension(id, key)
// SQL: _dim_has(dim_id, key) -> Long(0/1); _dim_get(dim_id, key) -> Long(value/0)
```

## watermark 语义（增强，不改 API）

- 事件时间模式：`watermark = max_seen_ts - lag_ms`（数据驱动）
- 触发：`end < watermark`（提前，事件时间推进）**OR** `end + lag <= now`（wall-clock 兜底 idle）
- 迟到丢弃：`end < watermark` 的窗口不入桶（超出乱序容忍的记录丢弃）
- 处理时间模式（无 ts 列）：行为同前（wall-clock）

## 验证记录

| 步骤 | 结果 |
|------|------|
| bpe-core 编译 | ✅ 零警告 |
| bpe-py-wrapper 编译 | ✅（PYO3_PYTHON=python3.11，pyo3 0.20 不支持 3.13） |
| bpe-java-wrapper 编译 | ✅ 零警告 |
| test_023_keyed_dimension（4 项） | ✅ per-key 分组、维表 WHERE(JIT)/SELECT(解释器)、watermark 迟到丢弃 |
| 全量测试 | ✅ 42 个测试二进制全部通过（零回归） |
| 性能基准 | ✅ filter 720ns / filter+agg 1454ns，与基线零回退 |
| Java 侧编译 | ✅（Bpe.java/JavaBpe.java 语法扩展，mvn 编译需 native 库环境） |

## 说明

- **Python 构建**：pyo3 0.20.3 最高支持 Python 3.12，需 `PYO3_PYTHON` 指向 3.11/3.12 解释器（系统默认 3.13 不可用）
- **Java 窗口回调**：复用 BpeCallback `([BI)V`，回调数据为聚合结果字节；窗口时间暂未暴露（后续版本可将 start/end 编码进回调）
- **窗口回调线程**：定时器线程执行，回调需线程安全（Send）
- 维表 key 为 Long 列；dim_id 为 SQL 常量参数
