# bpe-core 核心算法文档

> 生成日期：2026-08-15（full-analysis / code-deconstruct）

## 1. 环形缓冲窗口扫描（loop_filter）

### 算法描述

对每次 new_data，扫描环形缓冲中窗口范围内的记录，用 JIT 编译的过滤函数判定，命中的记录提取字段后写入回调缓冲，最多 LIMIT 条。

```
输入: WrappedArray (data, max_records=M, mask, walker=count, step)
      JIT filter, limit, asc
输出: 命中记录数 n, 回调缓冲内容

first_idx = (count-1 > M) ? (count-1-M) % M : 0
last_idx  = (count-1) % M
idx 迭代: asc ? 从 last_idx 递减至 first_idx : 从 first_idx 递增至 last_idx
position  = ((count-1-idx) * step) & mask
命中则: 求值字段 → 写入缓冲; n++; n==limit 提前退出
```

### 复杂度

- 每次 new_data：O(window × filter)，window ≤ M = vec_size/record_size = 2048（设计值）
- 累计 n 条记录：O(n²)（每记录重扫窗口）
- 优点：实现简单、cache 友好、JIT 过滤无解释开销

### 缺陷（已验证）

1. **回绕坍缩**：count > M+1 时 `first_idx == last_idx` 恒成立 → 每次只扫描 1 条最新记录。
   实测：写入 2048 条后 callback size 从 10 坍缩到 1。
   根因：`first_idx = (count-1-M) % M`，而 `(count-1-M) % M ≡ (count-1) % M = last_idx`。
   正确值应为 `(count-M) % M`（最旧有效记录）。
2. **方向语义混乱**：LIMIT 为正（asc=true）时从新到旧遍历但 position 计算使 idx=last_idx 对应 slot 0（最旧数据），实际命中顺序与声明不符。
3. `Idx::judge_or_step` 使用裸 `idx-1`，当 last_idx=0 且 first_idx≠0 时下溢为 usize::MAX → 理论死循环（当前因缺陷 1 而未被触发）。

## 2. SQL → LLVM JIT 过滤

### 表达式编译流程

```
WHERE expr → parse_exp 递归下降
  常量   → parse_val: {T_I64/T_F64, value}
  标识符 → gen_call_fetch_column: 调 fetch_i64/fetch_f64 (Rust extern fn)
  二元   → parse_binary_exp → bin_op_calc
    比较 → logic_compare: is_float 判断 → float/int 两路径 + PHI 合并
    逻辑 → logic_op: 直接 build_and/build_or（假定 0/1）
  一元 - → parse_negative_val: build_int_neg / build_float_neg
```

### 值表示

- 所有值统一为 `{type_tag: i64, value: i64}` 结构体（RetVal，repr(C)）
- 类型标签：T_ERR=0, T_B64=1, T_I64=2, T_F64=4
- float 以 bit pattern 存于 i64 槽位

### 缺陷

- `int2float`：`val as f64` 对负 i64 位模式产生巨大正浮点 → 混合类型比较（int vs float）结果错误
- AND/OR 对非 0/1 操作数（如裸列）位运算结果不可预测
- WHERE 未知列名 `unwrap()` panic（filter.rs:360）

## 3. 聚合计算

### 算法

```
init: 仅 MaxL=i64::MIN, MinL=i64::MAX, MaxD=f64::MIN, MinD=f64::MAX
      Sum/Count/Avg 不初始化（依赖 alloc 零页，非确定性）
compute: 对窗口内每条记录
  FirstL/FirstD: 取首条直接覆盖
  LastL/LastD:   取末条直接覆盖
  Max/Min/Sum/Count: fetch 累加/比较
  Avg: 增量均值 (avg*data_idx+v)/(data_idx+1)  ← 每次 new_data 从 data_idx=0 重新算窗口均值
```

### 缺陷

- Sum/Count/Avg 读未初始化内存（UB，实测恰好为 0 因零页）
- 聚合结果按 stream 列偏移存储：`stream.column(col_idx+1)` → stream 列数不足时 `get_unchecked` OOB panic（已验证 data.rs:261）
- Count 仅支持 Long 列；Double 列 count 静默失效
- sum/count 跨调用累积 + 全窗口重扫 → 二次增长（实测 a=1,2,3 → suml=1,4,10）
- 除零：`_div(x,0)` / `_mod(x,0)` panic → release abort

## 4. Executor 求值树

```
SELECT 字段 → ExprEntity → conv_as_executor
  Val(Bool/Int/Float)     → ConstLong/ConstDouble
  Field/FieldWithTab      → Fetch(record_id, field_id)
  Function("_xxx", args)  → Compute(SupportFunc, Executors(递归))
```

- 求值：`Executor::fetch → Element`；`Compute` 递归求 args 后按 SupportFunc 分派
- 类型提升：Long+Double→Double；结果按 Element.len()=8 字节写入
- 缺陷：`parse_args_fetchers` 出错时仅置标志后返回 Err，部分子表达式已入队（状态残留，但调用方随即失败返回，影响有限）

## 5. 内存寻址

- 列寻址：`base_ptr + column.offset`（定义期算好，零运行时开销）
- 字段类型 Long/Double 均 8 字节，offset = 列序 × 8
- 回调缓冲：命中记录按 step 紧凑排列，`param.step()` 为 record_size

## 6. 哈希/索引结构

- SimpleU16Map：`[u64; 65536]` 指针数组，O(1) 插入/查询，u16 id 直接索引
- bitmap（ID_STORE）：id/8 字节数组，位图标记已定义 id
- name→id：hashbrown::HashMap

## 7. 并发模型

- 无锁设计（单线程）：AtomicU16 计数器 + 全局指针
- new_data 非线程安全：Java 侧用单线程 MPSC 队列串行化；Rust 直接调用需外部串行
- 关键：多线程同时 new_data 会对 walker/缓冲造成数据竞争（未防护）
