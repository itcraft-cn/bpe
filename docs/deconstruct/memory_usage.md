# BPE 内存使用分析

> 生成日期：2026-08-15（full-analysis / code-deconstruct）

## 1. 内存布局总览

| 区域 | 大小 | 分配时机 | 释放 |
|------|------|----------|------|
| LLVM Context（PTR_LLVM_CTX_STORE） | ~MB 级 | start() | 永不（static 全局） |
| 每个 FuncGenerator（LLVM module + JIT engine） | 每 parse 一次 | 每次 def_mapper/def_aggregate 的 SQL 解析 | Box::leak，永不 |
| PTR_MAPPER_MAP（SimpleU16Map） | 65536×8 = 512KB 指针数组 | init_mapper | 永不 |
| mapper 的 PTR_VAL_DATA_REF | vec_size = 1MB | init_mapper | alloc 后永不 |
| aggregate 的 PTR_VAL_DATA_REF | 8192B | init_aggregate | 永不 |
| WrappedArray（每 record 一个） | vec_size = 1MB | 首次 new_data 懒创建 | 永不 |
| Executors（每 mapper/aggregate 一组） | 执行器数组 | define 时 | alloc 后永不 |
| SimpleU16Map 内元素（Record/Mapper/Aggregate） | 各对象 | 定义时 | Box::leak 永不 |
| U8Bytes（栈上，8192B 数组） | 8KB | 每次 new_data（调用方） | 栈 |

## 2. 内存热点

1. **WrappedArray 环形缓冲**：每记录 1MB 连续内存，单次分配，顺序访问，cache 友好（设计亮点）
2. **回调缓冲 PTR_VAL_DATA_REF**：mapper 1MB + aggregate 8KB，命中记录紧凑写入，复用无分配（亮点）
3. **Java 侧 conv_array**：每次回调 `new_byte_array(size*512)` → 高频 GC 压力（热点，Java 回调路径）
4. **Python 侧 conv_array**：每次回调新建 Vec<u8> → 同样热点
5. **JIT 过滤调用**：每记录一次 extern "C" 调用 + fetch_column（栈开销小）

## 3. 内存泄漏探测

### 泄漏（有意的"永不释放"设计，但需明确记录）

| 位置 | 泄漏内容 | 说明 |
|------|----------|------|
| aux.rs:77 `Box::leak` | SimpleU16Map 所有元素 | 全局注册表设计，进程生命周期有效 |
| sql/select.rs:26 `Box::leak` | 每次 parse 的 FuncGenerator（LLVM module+engine） | **每次 def_mapper 泄漏一个 LLVM 引擎**，多次定义累积 |
| store.rs:75 `alloc::alloc` | WrappedArray | 全局有效 |
| exec.rs:26 `alloc::alloc` | Executors 数组 | 全局有效 |
| mapper.rs:35 / aggregate.rs:38 `alloc::alloc` | VAL_DATA_REF | 全局有效 |

**风险**：`stop()` 只打日志（core.rs:47 "mark as deactived"），**不释放任何资源**；重复 def_mapper 同 SQL 会累积泄漏 JIT 引擎。若宿主进程长期运行且频繁动态定义 mapper，内存将线性增长。

### 泄漏（异常路径）

| 位置 | 场景 | 后果 |
|------|------|------|
| data.rs `insert_record` | 重复名称 def_incoming | record 已入 record_map（泄漏 id+对象），随后返回 None |
| aux.rs `insert` | 同一 id 二次 insert | 旧 Box 被覆盖且未释放（指针丢失） |

## 4. 内存安全风险

| 位置 | 风险 | 验证 |
|------|------|------|
| data.rs:74 `copy_from_slice(&slice[0..len])` | data_len>8192 时 `&data[0..size]` 越界读 → panic/UB | 静态分析 |
| store.rs `write_data` | data_len > record_size 时跨槽写入，污染相邻记录，缓冲末尾 OOB | 静态分析 |
| data.rs:261 `get_unchecked((column_id-1))` | column_id=0 或越界 → UB/panic | **已验证**（聚合 stream 列不足） |
| aux.rs `get_mut(&self) -> &'static mut T` | 共享引用产生 &mut，别名 UB | 静态分析 |
| aggregate.rs `init_for_some_func` | Sum/Count/Avg 读未初始化 alloc 内存（依赖零页） | 静态分析（实测碰巧为 0） |
| aggregate.rs size==0 路径 | 回调直接读未初始化/残留缓冲 | 静态分析 |
| jit/filter.rs:360 `column_id(name).unwrap()` | WHERE 未知列 → panic → release abort | 静态分析 |
| calc_func `div/mod_` 除零 | `_div(x,0)` panic → release abort | 静态分析 |
| store.rs `new` 无空指针检查 | OOM 时 null 解引用 | 静态分析 |

## 5. 改进建议

1. **优先**：new_data 校验 `data_len <= record_size`，否则拒绝；修复 U8Bytes 截断后 data_len 不同步问题
2. **优先**：聚合结果缓冲按聚合字段独立分配偏移（不依赖 stream 列布局），并初始化 Sum/Count/Avg 初值
3. `stop()` 释放 VAL_DATA_REF / WrappedArray / FuncGenerator，或明确文档化"进程级单例不释放"
4. FuncGenerator 复用：同 record 的多次定义复用同一 LLVM module
5. Java/Python conv_array 改为复用缓冲或零拷贝（DirectBuffer / memoryview）
6. SimpleU16Map 改用 Cell/RefCell 或拆分读写指针，消除 &mut 别名 UB
