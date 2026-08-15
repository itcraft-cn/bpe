# 修复计划：BPE 高速单表过滤/计算正确性修复

> 日期：2026-08-15（task-20260815-001）
> 依据：docs/detect/detect-20260815-001.md + docs/review/code-review-20260815-001-rust.md + docs/refactor/refactor-20260815-001.md

## 目标

修复已实证的正确性缺陷，使引擎在**长期运行 + 多规则 + 异常输入**下正确、不崩溃；保持公共 API 不变（`def_*` / `new_data` 签名不动），性能不回退。

## 修改点清单（最小化，逐项可独立验证）

| # | 缺陷 | 位置 | 修改类型 |
|---|------|------|----------|
| F1 | 环形缓冲回绕后窗口坍缩（实证：>2048 条后 size 10→1） | store.rs first_idx/last_idx + mapper.rs loop_filter | 索引改为"写序号"语义，position=(idx*step)&mask |
| F2 | 聚合 Sum/Count/Avg 读未初始化内存 | aggregate.rs init_for_some_func | 补 SumL/SumD/Count/Avg/First/Last 初值 |
| F3 | 聚合跨调用累积 + 全窗口重扫（实证：sum=1→4→10） | aggregate.rs call_with_aggregate_data | size==0 时也先 init 再回调；窗口内语义统一 |
| F4 | 聚合结果按 stream 列偏移存储 → OOB panic（实证 data.rs:261） | aggregate.rs setup_init_val/compute_data | 独立偏移 col_idx*8 |
| F5 | mapper 按 mapper id 存、按 record id 查 → 同表多规则失效 | mapper.rs define_mapper/call_mapper | record_id → Vec\<Mapper\> 列表 |
| F6 | def_aggregate 恒返回 Some(1) → 第二个聚合不可绑定 | aggregate.rs define_aggregate | 返回真实 id |
| F7 | 聚合输入按 stream 列偏移读 mapper 输出 → 值错位 | mapper.rs + aggregate.rs | 绑定期按**字段名**解析 mapper 输出偏移 |
| F8 | WHERE 未知列 unwrap → release abort | jit/filter.rs:360 | 返回 issue + None |
| F9 | data_len > record_size 越界写槽 | core.rs new_data + data.rs U8Bytes | new_data 校验 + U8Bytes 截断同步 |
| F10 | 除零 panic → abort | element.rs div/mod_ | 除零返回 0 |
| F11 | int2float 负 i64 转巨大正浮点 | jit/aux.rs:78 | signed 转换 |
| F12 | 重复 record 名残留僵尸记录 | data.rs insert_record | 先查重再入表 |
| F13 | Record::column get_unchecked OOB | data.rs:261 | 边界守卫（warn + 兜底） |
| F14 | Count 不支持 Double 列 | aggregate.rs choose_func | Count 匹配任意元素 |

## 明确不做（最小化原则）

- SimpleU16Map `get_mut` 别名 UB：单线程顺序使用，改动风险>收益，文档化约束
- stop() 资源释放 / JIT 引擎复用：进程级单例设计，超出本次范围
- Java/Python 封装 512 常量：默认 record_size=512 下行为正确
- 全窗口重扫 O(n²)：窗口语义下 LIMIT 提前退出已控开销

## 验证计划

1. 新增回归测试 bpe-test/tests/test_020_regression.rs（6 项断言测试）
2. 全量回归：cargo test（bpe-core 单测 + bpe-test 18 个 + lib_test 15 个）
3. 性能抽查：cargo bench（filter / filter+aggregate）对比修复前后
4. 每项修复独立提交，可回滚

## 实施顺序

F1 → F5/F6 → F7 → F2/F3/F4/F14 → F8 → F9 → F10 → F11 → F12/F13 → 测试 → 回归 → bench
