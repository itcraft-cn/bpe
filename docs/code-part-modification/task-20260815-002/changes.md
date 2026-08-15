# 修改与验证记录

> task-20260815-002：添加主要 SQL 函数

## 修改清单

| # | 文件 | 修改内容 |
|---|------|----------|
| C1 | func_enum.rs | SupportFunc 增加 19 个变体；显式 `serialize` 保持既有 SQL 名（suml/minl/maxl 等）兼容；Display 提供 snake_case 名供 JIT 解析 |
| C2 | calc_func.rs | 重构为"纯元素级逻辑（`*_elem`）+ 解释器包装"双层；新增 15 个标量函数（abs/ceil/floor/round/trunc/sign/sqrt/exp/ln/log10/to_long/to_double/pow/greatest/least），纯逻辑与 JIT 共享 |
| C3 | exec.rs | compute_func 分派新增 15 个标量函数 |
| C4 | agg_func.rs | 新增 Welford 在线算法：f_stddev_step / f_stddev_finalize / f_var_finalize（支持总体/样本） |
| C5 | aggregate.rs | 新增 4 个聚合（Stddev/StddevSamp/Variance/VarSamp）：init 清零状态区、choose_func 步进、compute_data 后 finalize 写回；状态区独立于输出槽（AGG_STATE_BASE=4096，每函数 24B），外部回调布局不变 |
| C6 | jit/aux.rs | JIT wrapper（宏生成 unary/binary extern "C" fn）+ element_from_bits/retval_from_element 转换 |
| C7 | jit/base.rs | register_scalar_funcs：20 个标量函数注册到 LLVM 模块（`func_{name}`），WHERE 条件可调用 |
| C8 | jit/filter.rs | parse_exp 新增 Function 分支；parse_function 递归解析参数、调用注册函数、返回 {type,value} 结构 |
| C9 | bpe-test/tests/test_021_sql_funcs.rs | 新增 4 项断言测试 |

## 新增 SQL 函数

### 标量（SELECT 字段 + WHERE 条件均可）

| SQL | 说明 | SQL | 说明 |
|-----|------|-----|------|
| `_abs(x)` | 绝对值 | `_sign(x)` | 符号 |
| `_ceil(x)` | 向上取整 | `_trunc(x)` | 截断 |
| `_floor(x)` | 向下取整 | `_to_long(x)` | 转 i64 |
| `_round(x)` | 四舍五入 | `_to_double(x)` | 转 f64 |
| `_sqrt(x)` | 平方根→f64 | `_pow(x,y)` | 幂→f64 |
| `_exp(x)` | e^x→f64 | `_greatest(x,y)` | 取大 |
| `_ln(x)` | 自然对数→f64 | `_least(x,y)` | 取小 |
| `_log10(x)` | 常用对数→f64 | | |

### 聚合

| SQL | 说明 |
|-----|------|
| `_stddev(x)` | 总体标准差（f64） |
| `_stddev_samp(x)` | 样本标准差（f64） |
| `_variance(x)` | 总体方差（f64） |
| `_var_samp(x)` | 样本方差（f64） |

## 验证记录

| 步骤 | 结果 |
|------|------|
| 编译（debug） | ✅ 零错误零警告 |
| test_021_sql_funcs（4 项） | ✅ 全过：标量 SELECT、WHERE 函数（含嵌套）、stddev 四聚合、Double 列 stddev+count |
| 全量测试 | ✅ 40 个测试二进制全部通过（3 单测 + 12 lib + 18 test + 8 回归 + 4 函数 + 其余） |
| 性能基准 | ✅ filter 627ns / filter+agg 1380ns，与基线零回退 |
| 实机验证（bin） | ✅ stddev=0.816(sum=6,avg=2, a=1,2,3) |

## 关键设计点

1. **解释器/JIT 双路径共享纯逻辑**：`calc_func::*_elem` 为唯一实现，exec.rs（SELECT 求值）与 jit wrapper（WHERE 过滤）均委托之，保证两路径语义一致
2. **函数名兼容**：显式 serialize 保持既有 `_suml/_minl/_maxl/_firstl/...` 不变，新函数用 snake_case（`_stddev_samp`/`_to_double` 等）
3. **WHERE 函数调用**：`_name(args)` 编译为对注册 Rust 函数 `func_{name}` 的 LLVM call，参数以 {type_tag, value} 对传递，返回值同构，可任意嵌套与比较
4. **聚合状态隔离**：Welford 状态存聚合缓冲独立区域，输出槽布局不变（外部回调兼容）

## 不做（文档化）

- 字符串函数：类型系统仅 Long/Double，需架构级扩展
- 聚合参数表达式（`_suml(_abs(a))`）：保持单字段参数
- 变参 GREATEST/LEAST：保持 2 参，可嵌套
- 说明：n=1 时 stddev/var 输出 0（未定义，样本方差 n<2 亦为 0）
