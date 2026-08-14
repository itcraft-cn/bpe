# bpe-core 模块设计文档

> 生成日期：2026-08-15（full-analysis / code-deconstruct）
> 源码规模：bpe-core/src 共 34 个 .rs 文件，约 4150 行

## 模块清单

| 模块 | 文件 | 职责 |
|------|------|------|
| lib | lib.rs | 模块声明 + 公共 API 导出 |
| core | core.rs | 生命周期 start/stop、定义入口、new_data 入口 |
| data | data.rs | Record/Column/U8Bytes 数据定义与注册表 |
| store | store.rs | WrappedArray 环形缓冲存储 |
| mapper | mapper.rs | SQL→Mapper、窗口扫描 loop_filter、回调分发 |
| aggregate | aggregate.rs | 聚合函数定义与计算 |
| exec | exec.rs | Executor 执行树（常量/取列/计算） |
| calc_func | calc_func.rs | 二元算术（add/sub/mul/div/mod） |
| agg_func | agg_func.rs | 聚合原语（max/min/sum/count/avg/first/last） |
| element | element.rs | Element 值类型（Long/Double）及算术 |
| func_enum | func_enum.rs | SupportFunc 函数枚举（strum 解析） |
| sql/base | sql/base.rs | SQL 解析选项、ParsedSql、ExprEntity |
| sql/select | sql/select.rs | SELECT 语句解析、字段/过滤/限制 |
| jit/base | jit/base.rs | FuncGenerator（LLVM 模块/JIT 引擎） |
| jit/filter | jit/filter.rs | WHERE→LLVM IR 生成 |
| jit/llvm_misc | jit/llvm_misc.rs | 比较/逻辑运算 IR 生成 |
| jit/aux | jit/aux.rs | JIT 调用的 Rust 侧函数（fetch_column/int2float） |
| jit/log | jit/log.rs | JIT 调试日志 |
| aux | aux.rs | 底层内存读写、bitmap、SimpleU16Map |
| cfg | cfg.rs | TOML 配置加载 |
| logger | logger.rs | log4rs 日志（滚动文件/控制台） |
| id | id.rs | 全局递增 ID（AtomicU16） |
| param | param.rs | CallbackParams（回调参数） |
| callback | callback.rs | FnHolder 枚举与统一分发 |
| ffi | ffi.rs | FfiFunc trait（跨语言回调） |
| consts | consts.rs | 常量定义 |
| error | error.rs | ParseSqlError |
| macros | macros.rs | 通用日志/断言宏 |
| utest | utest/base.rs | 单元测试初始化（硬编码 BPE_HOME） |

## 1. 数据定义模块（data.rs）

### Record / Column / U8Bytes

- **U8Bytes**：固定 8192 字节数组的输入数据载体；`new_from_vec` 拷贝时若 len>8192 截断，但 **data_len 不随截断修正**（data_len>8192 时 insert 阶段越界读）。
- **Column**：name/type(idx 0=Long,1=Double)/idx(1 基)/offset；`copy_from_columns` 计算偏移，Long/Double 均 8 字节。
- **Record**：record 注册表；`insert_record` 顺序为"先插入 record_map 再查重 name_map"→ **重复名称时残留僵尸记录**；`column()` 用 `get_unchecked((column_id-1))` → column_id 越界即 UB。

### 全局状态
`static mut PTR_RECORD_MAP/PTR_NAME_MAP/PTR_ID_STORE: u64` + globalvar crate 托管，`init_data` 中初始化。

## 2. 存储模块（store.rs）

### WrappedArray 环形缓冲

```rust
struct WrappedArray { data: *mut u8, max_records, mask, walker, step }
```

- `new`：`alloc::alloc` 一次性分配 vec_size 字节（**无失败检查、永不释放**）
- 写入：`(walker & mask) * step` 定位 + `copy_nonoverlapping` + walker+=step
- `mask = size-1`：**要求 vec_size 为 2 的幂**，否则寻址错误（默认 1MB=2^20 恰好合规）
- 窗口索引 `first_idx/last_idx`：**回绕算术错误**（实测：>2048 条后窗口坍缩为 1 条）

## 3. Mapper 模块（mapper.rs）

### 定义与调用

- `define_mapper`：parse_select → gen_mapper（create_executor）→ 存入 `PTR_MAPPER_MAP`
  - **key 用 mapper.id()（next_mapper_id），查询用 record id** → 同一 record 的第二个 mapper 永远不被调用
- `call_mapper(array, id)`：search_mapper(record_id) + Record::get_record(record_id)
- `loop_filter`：窗口扫描主循环
  - 迭代 `Idx`：asc(LIMIT>=0) 从 last_idx 递减到 first_idx；否则反向
  - `position = ((walker-1-idx)*step) & mask`；JIT filter 判定；命中→`mapper.fetch` 求值字段
  - LIMIT 提前退出
- `invoke`：回调参数 `CallbackParams::new(u8_ptr, mask, offset, size, step)`

### 字段求值
`Mapper::fetch`：按 SELECT 字段顺序逐个 Executor 求值，结果按 `Element.len()` 写入回调缓冲（offset 累加）。

## 4. Aggregate 模块（aggregate.rs）

### 定义与调用

- `define_aggregate`：parse_select → gen_aggregate → 存入 `PTR_AGGREGATE_MAP`
  - **返回 `Some(1)` 硬编码**，非真实 aggregate id → 第二个聚合无法通过 def_mapper_bind_aggregate 正确绑定
- `call_aggregate(wrapped, param)`：由绑定 mapper 的 Lambda 回调触发
- `call_with_aggregate_data`：
  - param.size()==0 → 直接回调（size=0，缓冲未初始化/残留）
  - 否则 init_data → compute_data → 回调（**size 恒为 1**）
- `init_data`：仅对 MaxL/MinL/MaxD/MinD 初始化极值；**SumL/SumD/Count/Avg 未初始化**（依赖 alloc 零页的偶然行为，非确定性）
- `compute_data`：按聚合字段索引 col_idx → `stream.column(col_idx+1)` 取偏移 → **stream 列数不足时 OOB panic**
- First/Last：首/末条；其余聚合：loop_compute 逐条 choose_func
- Count 只匹配 Element::Long → **Double 列 count 静默失效**

### 聚合语义（实测）
- sum/count **跨 new_data 调用累积不重置**，且每调用重扫全窗口 → 二次增长
- avg 用 `(avg*data_idx+v)/(data_idx+1)` 增量公式，恰好为窗口内均值

## 5. 执行树模块（exec.rs / calc_func.rs / element.rs）

```
Executor 枚举：
  ConstLong(i64) | ConstDouble(f64) | Fetch(record_id, field_id) | Compute(SupportFunc, Executors)
```

- `create_executor`：ExprEntity → Executor 递归转换；`_` 前缀函数映射到 SupportFunc
- `Executors::new`：alloc::alloc 拷贝执行器数组（**永不释放**）
- `compute_func`：Add/Sub/Mul/Div/Mod → calc_func
- Element 算术：Long/Double 混合提升；`div`/`mod_` **除零 panic → release 下 abort**
- SupportFunc：`_add/_sub/_mul/_div/_mod/_minl/_maxl/_suml/_count/_mind/_maxd/_sumd/_avg/_firstl/_firstd/_lastl/_lastd`

## 6. SQL 解析（sql/base.rs + sql/select.rs）

- sql-parse crate，MariaDB 方言，`?` 参数模式
- 仅支持：单表 SELECT + WHERE + LIMIT（正数=新→旧、负数=旧→新）
- 禁止：JOIN、GROUP BY、ORDER BY、HAVING、子查询、`AS`、offset、`*`、CASE、CAST、IN、函数（除 `_` 前缀）
- 字段解析：`parse_expr` → ExprEntity；WHERE 解析：JIT 编译（见下）
- **WHERE 未知列名 → `record.column_id(name).unwrap()` panic**（filter.rs:360），release 下 abort

## 7. JIT 模块（jit/*）

### FuncGenerator（jit/base.rs）

- 每次 `parse_select_statement` 创建新的 `FuncGenerator`（新 LLVM module + JIT engine），`Box::leak` **永不释放**
- 共享全局 LLVM Context（`PTR_LLVM_CTX_STORE`）
- 注册 Rust 侧函数：fetch_i64/fetch_f64/i2f/logint/logfloat（add_global_mapping）
- 编译 `record_filter_{record_id}` → `JitFunction<FilterFunc>`（`unsafe extern "C" fn(u64)->bool`）

### 过滤 IR 生成（jit/filter.rs + llvm_misc.rs）

- 表达式 → `{type, value}` 结构体（RetVal 布局：i64×2）
- 类型标签：T_ERR=0, T_B64=1, T_I64=2, T_F64=4
- 比较：float/int 双路径 + PHI 合并；mixed 类型经 `i2f` 转换
- `i2f` 实现缺陷：**负 i64 经 `val as f64` 变为巨大正浮点**（应以 signed 转换）
- AND/OR：直接对 i64 位运算（假定操作数为 0/1）
- 取列：`fetch_column_i64/f64`（extern "C"）按 Column.offset 读内存，返回 RetVal

### 关键交互

```
JIT 过滤函数签名：unsafe extern "C" fn(data_ptr: u64) -> bool
loop_filter 中调用：filter.call(v_sub_ptr)
```

## 8. 基础设施

### SimpleU16Map（aux.rs）

- `[u64; 65536]` 指针数组，u16 id → Box::leak 指针
- **`get_mut(&self) -> &'static mut T`：从共享引用产生 &mut，违反 Rust 别名规则（UB）**
- insert 覆盖时旧 Box 泄漏

### cfg / logger

- TOML 配置：dev_mode/vec_size/record_size/log_dir；BPE_HOME 环境变量定位
- log4rs：dev=控制台+滚动文件，prod=仅滚动文件（200MB×20）

### id.rs

- 三个独立 AtomicU16 计数器（SeqCst）：record/mapper/aggregate 从 1 起

## 9. 跨语言封装

### bpe-java-wrapper（JNI）

- 导出 Java_cn_itcraft_bpe4j_Bpe_* 8 个 native 方法
- JavaFfiFunc：持 JavaVM + GlobalRef，回调时 `get_env` + call_method("callback","([BI)V")
- **U8_DATA_MAX_SIZE 硬编码 512，与 core 的 8192/可配置 record_size 不一致** → record_size≠512 时回调数据错位
- Java 侧：JavaBpe（ByteConverter 注册表）+ JavaBpeThread（MPSC 10240 队列 + AdaptiveWaitStrategy）单线程消费，UnsafeUtil/Bytes 用 sun.misc.Unsafe 编解码（仅 Java 8）

### bpe-py-wrapper（PyO3）

- bpe4py 模块 8 个函数；PythonFfiFunc 回调持 PyObject，with_gil 调 callback(bytearray)
- 同样硬编码 512

## 10. 测试/基准

- bpe-core/tests：15 个 lib_test_XX.rs（SQL 解析/过滤功能）
- bpe-test/tests：18 个 test_XXX.rs（性能测量型，几乎全为复制粘贴模板，仅 SQL/LOOP_SIZE 不同）
- benches：benchmarks1/2（criterion，filter / filter+aggregate）
- **测试只测耗时不测结果正确性**（无断言），边界缺陷无法被发现
