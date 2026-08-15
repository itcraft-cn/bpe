# 修改与验证记录

> task-20260815-001：BPE 高速单表过滤/计算正确性修复

## 修改清单（changes.md）

| # | 文件 | 修改内容 | 对应缺陷 |
|---|------|----------|----------|
| C1 | store.rs | `first_idx` 改为"最旧写序号"（count-M），`last_idx` 改为"最新写序号"（count-1） | F1 窗口回绕坍缩 |
| C2 | mapper.rs loop_filter | position 改为 `(write_idx * step) & mask`，窗口按写序号迭代（回绕后仍扫描全部 M 条） | F1 |
| C3 | mapper.rs | mapper map 改按 **record id** 存储 `WrappedMapperList`（Vec），`call_mapper` 遍历执行全部规则 | F5 多规则失效 |
| C4 | mapper.rs | 新增 `define_mapper_bind_aggregate`：绑定期按**字段名**解析聚合输入偏移（resolve_aggregate_offsets） | F7 聚合值错位 |
| C5 | aggregate.rs | `define_aggregate` 返回真实 aggregate id | F6 多聚合不可绑定 |
| C6 | aggregate.rs | 聚合结果改独立偏移 `col_idx * FIELD_SIZE`，不再依赖 stream 列布局 | F4 OOB panic |
| C7 | aggregate.rs | `init_for_some_func` 补 SumL/SumD/Count/Avg/First/Last 初值；`init_data` 恒在回调前执行（含 size==0 路径） | F2 未初始化读、F3 窗口语义 |
| C8 | aggregate.rs | Count 匹配任意列类型（不再只支持 Long） | F14 |
| C9 | aggregate.rs | fetch_arg_val 用解析偏移读 mapper 输出（类型仍取 stream 列） | F7 |
| C10 | core.rs | `new_data` 校验 `data_len <= record_size`，超限拒绝 | F9 越界写槽 |
| C11 | data.rs | U8Bytes::new_from_vec/slice 截断后同步 data_len（防 `&data[0..size]` 越界） | F9 |
| C12 | data.rs | `insert_record` 先查重名再入表（防僵尸记录） | F12 |
| C13 | data.rs | `Record::column` 越界守卫（warn + 兜底，防 release abort） | F13 |
| C14 | element.rs | div/mod 除零守卫（返回 0） | F10 |
| C15 | jit/aux.rs | int2float 按有符号解释 i64（负整数不再变巨大浮点） | F11 |
| C16 | jit/filter.rs | WHERE 未知列名返回 issue + None（不再 unwrap panic） | F8 |
| C17 | jit/filter.rs | JIT 过滤函数名加全局唯一序号（同 record 多 mapper 共享 LLVM Context 下的符号隔离加固） | 防御性 |
| C18 | bpe-test/tests/test_020_regression.rs | 新增 8 项断言回归测试（引擎全局串行锁 + 抗中毒） | 验证 |

## 验证记录（verification.log）

| 步骤 | 命令 | 结果 |
|------|------|------|
| 编译（debug） | `RUSTFLAGS="-lLLVM-19" cargo build` | ✅ 零错误零警告 |
| 回归测试（8 项） | `cargo test -p bpe-test --test test_020_regression` | ✅ 8/8 通过 |
| 全量测试 | `RUSTFLAGS="-lLLVM-19" cargo test` | ✅ 3 单测 + 12 lib_test + 18 test_XXX + 8 回归 全部通过 |
| 性能基准 | `cargo bench benchmarks1 benchmarks2` | ✅ filter 667ns / filter+agg 1503ns，与基线**零回退** |
| 原始 OOB 复现 | 1 列 stream + 3 聚合字段 | ✅ sum=6 count=3 avg=2，不再 panic |

## 验证亮点

1. **F1 窗口修复**：3000 条连续写入后回调 size 恒为 10（修复前坍缩为 1）
2. **F2/F3 聚合语义**：a=1,2,3 → sum=6/count=3/avg=2（修复前 sum=1→4→10 二次增长）
3. **F7 字段名解析**：mapper `SELECT f, a` + 聚合 `_suml(stream.a)` → 读 a 的值（修复前读 f）
4. **F5/F6 多规则**：同 record 2 mapper 分别命中；2 个聚合各自可绑定
5. **F8-F10 防崩溃**：未知列/超长数据/除零均不崩溃（返回 None/false/0）

## 明确不做（文档化约束）

- SimpleU16Map `get_mut` 别名：单线程顺序使用，维持现状（已注释说明）
- stop() 资源释放 / JIT 引擎复用：进程级单例设计，另行评估
- Java/Python 封装 512 常量：默认 record_size=512 下行为正确
