# 计划：添加主要 SQL 函数

> 日期：2026-08-15（task-20260815-002）
> 目标：在已修复正确性的 BPE 引擎上扩展 SQL 函数能力，服务"高速单表过滤/计算"

## 新增函数

### 标量函数（SELECT 字段 + WHERE 条件，两处均可用）

| SQL 名称 | SupportFunc | 语义 | 元数 |
|----------|-------------|------|------|
| `_abs` | Abs | 绝对值 | 1 |
| `_ceil` | Ceil | 向上取整 | 1 |
| `_floor` | Floor | 向下取整 | 1 |
| `_round` | Round | 四舍五入 | 1 |
| `_sqrt` | Sqrt | 平方根（→Double） | 1 |
| `_exp` | Exp | e^x（→Double） | 1 |
| `_ln` | Ln | 自然对数（→Double） | 1 |
| `_log10` | Log10 | 常用对数（→Double） | 1 |
| `_sign` | Sign | 符号（-1/0/1） | 1 |
| `_trunc` | Trunc | 截断小数 | 1 |
| `_to_long` | ToLong | 转 i64 | 1 |
| `_to_double` | ToDouble | 转 f64 | 1 |
| `_pow` | Pow | 幂（→Double） | 2 |
| `_greatest` | Greatest | 取大 | 2 |
| `_least` | Least | 取小 | 2 |

### 聚合函数

| SQL 名称 | SupportFunc | 语义 |
|----------|-------------|------|
| `_stddev` | Stddev | 总体标准差（Welford 在线算法） |
| `_stddev_samp` | StddevSamp | 样本标准差 |
| `_variance` | Variance | 总体方差 |
| `_var_samp` | VarSamp | 样本方差 |

## 架构设计

### 1. 纯逻辑与执行器解耦（calc_func.rs）
- 元素级纯函数 `abs_elem(Element) -> Element` 等：**解释器（exec.rs）与 JIT 共用同一实现**
- 解释器包装（带 executors 取参）复用现有 fetch_2_arg 模式，新增 fetch_1_arg

### 2. WHERE 中函数调用（JIT）
- 现状：`parse_exp` 对 Function → unsupported（WHERE 中不能用函数）
- 方案：每个标量函数注册为 LLVM 模块内的 Rust 侧函数（`func_{name}`），签名 `(t1,v1[,t2,v2]) -> RetVal{type,value}`，与现有 fetch_column_i64 模式一致
- `parse_exp` 加 Function 分支：递归解析 args → 提取 {type,value} → build_call → 返回值转 LLVM struct

### 3. 聚合扩展（stddev/var）
- Welford 在线算法：状态 (count, mean, m2) 24 字节，存聚合缓冲专用状态区（offset 4096 起，col_idx*24）
- 输出槽仍为 col_idx*8（外部布局不变）；compute_data 循环后 finalize 写回
- 每次 new_data 前 init 清零状态（窗口语义，与 sum/avg 一致）

## 不做（文档化）

- 字符串函数：当前类型系统仅 Long/Double，需架构级扩展，另行评估
- 聚合参数表达式（如 `_suml(_abs(a))`）：保持单字段参数
- 变参 GREATEST/LEAST（N 参）：保持 2 参，可用嵌套

## 验证

1. 新增 test_021_sql_funcs.rs：标量函数 SELECT 求值、WHERE 函数过滤、聚合 stddev/var 断言
2. 全量回归 + 性能基准零回退
