# BPE 核心数据流

> 生成日期：2026-08-15（full-analysis / code-deconstruct）

## 1. 数据流总览

```
┌──────────┐   new_data(&U8Bytes)   ┌──────────────────────────┐
│ 外部调用方 │ ───────────────────────▶│  core::new_data          │
│ Rust/Java/│                        │  ├ check_id_in_store(id)  │
│ Python    │                        │  └ find_or_insert_array   │
└──────────┘                        └────────────┬─────────────┘
                                                 ▼
                                    ┌──────────────────────────┐
                                    │  store::insert           │
                                    │  环形缓冲 WrappedArray    │
                                    │  base=(walker&mask)*step  │
                                    │  walker += step          │
                                    └────────────┬─────────────┘
                                                 ▼
                                    ┌──────────────────────────┐
                                    │  mapper::call_mapper(id)  │
                                    │  search_mapper(record_id) │
                                    └────────────┬─────────────┘
                                                 ▼
                                    ┌──────────────────────────┐
                                    │  loop_filter 窗口扫描      │
                                    │  last_idx→first_idx 遍历   │
                                    │  JIT filter.call(ptr)     │
                                    └────────────┬─────────────┘
                                                 ▼
                                    ┌──────────────────────────┐
                                    │  mapper.fetch 提取字段     │
                                    │  Executor 求值→VAL 缓冲    │
                                    └────────────┬─────────────┘
                                                 ▼
                                    ┌──────────────────────────┐
                                    │  callback(FnHolder)       │
                                    │  ├ Func: Rust 闭包         │
                                    │  ├ FfiFunc: Java/Python   │
                                    │  └ Lambda: 聚合回调        │
                                    └──────────────────────────┘
```

## 2. 数据生命周期

### 2.1 定义阶段（启动时一次性）

1. `start()` → `load_config → init_logger → init_func_generator(LLVM ctx) → init_data → init_store → init_mapper → init_aggregate`
2. `def_incoming(name, columns)` / `def_stream(name, columns)` → `Record::insert_record`：
   - 分配全局递增 record id（`id.rs`，从 1 开始）
   - 计算列偏移（每列固定 8 字节：`Column::copy_from_columns`）
   - 注册到 `PTR_RECORD_MAP`（SimpleU16Map：u16 id → Box::leak 指针）
   - 注册 name → id 到 `PTR_NAME_MAP`，bitmap 置位 `PTR_ID_STORE`
3. `def_mapper(sql, fn)` → `parse_select`（sql-parse crate，MariaDB 方言）：
   - 解析 FROM 单表（不支持 JOIN/多表/子查询）
   - WHERE 条件 → `gen_select_filter_func` → LLVM JIT 编译为 `extern "C" fn(u64)->bool`
   - SELECT 字段 → `ExprEntity` 列表 → `create_executor` → `Executors`
   - 存储到 `PTR_MAPPER_MAP`（**key 为 mapper id，而非 record id**）
4. `def_aggregate(sql, fn)`：类似 mapper，生成聚合执行器，存 `PTR_AGGREGATE_MAP`
5. `def_mapper_bind_aggregate(sql, agg_id)`：mapper 回调绑定聚合函数

### 2.2 运行阶段（每条记录）

```
new_data(U8Bytes)
  → bitmap 检查 id 是否已定义
  → find_or_insert_array(id)：懒分配 1MB 环形缓冲
  → insert：memcpy 到 (walker & mask)*step，walker+=step
  → call_mapper(id)
      → search_mapper(record_id)   ← 注意：用 record id 查 mapper
      → loop_filter：
          idx 从 last_idx 递减到 first_idx
          position = ((walker-1-idx)*step) & mask
          filter.call(sub_data_ptr)   ← JIT 编译的过滤函数
          命中 → Executor 求值 SELECT 字段 → 写入 VAL 缓冲
          n==limit 提前退出
      → callback(FnHolder, CallbackParams{u8_ptr, mask, offset, size, step})
          - 普通 mapper：用户闭包直接消费
          - 绑定聚合：call_aggregate → init_data → compute_data → 聚合回调(size=1)
```

### 2.3 关键参数

| 参数 | 默认值 | 位置 |
|------|--------|------|
| vec_size（环形缓冲总大小） | 1048576 (1MB) | cfg/config.toml |
| record_size（单条记录步长） | 512（上限 8192） | cfg/config.toml |
| max_records（窗口容量） | vec_size/record_size = 2048 | store.rs |
| LIMIT（每次回调最大条数） | 10 | SQL / DEFAULT_SELECT_SIZE |
| U8_DATA_MAX_SIZE | 8192 | consts.rs |

## 3. 数据布局

### 3.1 环形缓冲（WrappedArray）

```
┌─────────────────────────── vec_size (1MB) ───────────────────────────┐
│ Record0 │ Record1 │ ... │ Record2047 │ Record0(覆写) │ Record1(覆写) │ ...
└──────────────────────────────────────────────────────────────────────┘
   ▲ walker*step 写入位置（掩码回绕）
   每记录 step=512 字节，列偏移固定：Long/Double 各 8 字节
```

### 3.2 回调数据缓冲（PTR_VAL_DATA_REF）

```
loop_filter 命中记录按 step 紧凑排列：
┌──────────┬──────────┬──────────┐
│ match[0] │ match[1] │ ...      │  ← size = n (≤ limit)
└──────────┴──────────┴──────────┘
```

### 3.3 聚合结果缓冲（aggregate PTR_VAL_DATA_REF，8192 字节）

按 **stream 列偏移** 存储聚合结果：第 i 个聚合字段写在第 i+1 列的 offset 处。

## 4. 跨语言数据流（JNI / PyO3）

```
Java: newDataAsync(id, data) → MPSC 队列 → JavaBpeThread → Bpe.newData → JNI → Rust new_data
      Rust 回调 → JavaFfiFunc::callback → JVM call_method("callback", "([BI)V")
Python: new_data(id, bdata) → PyO3 → Rust new_data
      Rust 回调 → PythonFfiFunc::callback → Python::with_gil → callback(bytearray)
```

## 5. 已验证的行为特性（2026-08-15 实测）

1. **窗口扫描在回绕后坍缩**：写入 >2048 条记录后，每次 new_data 只扫描 1 条最新记录（callback size 从 10 掉到 1）。根因：`first_idx` 的 `(count-1-max_records) % max_records` 与 `last_idx` 算术在回绕后恒等，迭代区间坍缩。
2. **聚合跨调用累积**：sum/count 不重置，每次 new_data 重扫窗口并累加（a=1,2,3 时 suml=1→4→10，呈二次增长）。avg 因使用 data_idx 增量公式而恰好正确。
3. **聚合结果按 stream 列偏移存储**：stream 列数 < 聚合字段数时触发 `get_unchecked` OOB panic（data.rs:261）。
