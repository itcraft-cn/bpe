# BPE 架构设计总结

> 生成日期：2026-08-15（full-analysis / code-deconstruct）

## 1. 架构模式识别

| 模式 | 应用位置 | 说明 |
|------|----------|------|
| **管道-过滤器**（Pipeline-Filter） | 核心数据路径 | new_data → store → mapper(filter) → callback，数据单向流动 |
| **单例**（Singleton） | 全局状态 | globalvar + static mut 指针，Once 初始化 |
| **注册表**（Registry） | Record/Mapper/Aggregate | SimpleU16Map id 索引全局注册 |
| **模板方法** | FnHolder 回调分发 | Func/FfiFunc/Lambda 统一 callback 入口 |
| **策略** | 聚合分派 | choose_func 按 SupportFunc 分派 |
| **JIT 编译** | WHERE 过滤 | LLVM IR 生成 + 执行引擎映射 |

## 2. 架构分层

```
┌────────────────────────────────────────────────┐
│ API 层：core.rs（start/def_*/new_data）        │
├────────────────────────────────────────────────┤
│ 定义层：data.rs（Record/Column 注册表）        │
├────────────────────────────────────────────────┤
│ SQL 层：sql/select.rs + sql/base.rs            │
│   └→ JIT 层：jit/*（LLVM IR 生成 + 执行）      │
├────────────────────────────────────────────────┤
│ 执行层：exec.rs + calc_func.rs + element.rs    │
│   Mapper 层：mapper.rs（窗口扫描/回调）         │
│   Aggregate 层：aggregate.rs + agg_func.rs     │
├────────────────────────────────────────────────┤
│ 存储层：store.rs（WrappedArray 环形缓冲）      │
├────────────────────────────────────────────────┤
│ 基础设施：aux/cfg/logger/id/consts/error       │
└────────────────────────────────────────────────┘
```

## 3. 设计亮点（值得肯定）

1. **性能优先的存储设计**：
   - 环形缓冲单次分配、固定步长、顺序访问 → cache 友好（1MB 缓冲 40B 结构体单 cache line）
   - 列偏移定义期预计算 → 运行时零查找开销
   - 固定大小记录 → 无动态分配、可预测访问模式
2. **JIT 过滤**：WHERE 编译为原生机器码，运行时无解释开销；类型标签 + PHI 合并的 int/float 双路径比较设计思路清晰
3. **回调缓冲复用**：PTR_VAL_DATA_REF 单缓冲复用，避免每记录分配
4. **跨语言封装解耦**：FfiFunc trait 统一 Rust/Java/Python 回调，核心不依赖 JNI/PyO3
5. **代码组织**：模块职责单一、命名清晰、注释充分（中文注释详尽）

## 4. 架构缺陷（问题导向）

### 4.1 全局状态滥用

- 10+ 处 `static mut` + globalvar 指针：类型安全归零，所有跨模块共享都走裸指针
- `get_mut(&self) -> &'static mut T` 违反 Rust 别名规则 → 已潜伏 UB
- 单例 + Once：无法重启、无法多实例、无法测试隔离

### 4.2 身份体系断裂（关键设计缺陷）

- **record / mapper / aggregate 三个独立 id 计数器**，但 mapper map 用 mapper id 存、record id 查；aggregate 返回硬编码 Some(1)
- 结果：同一 record 的多个 mapper 只有第一个生效；多个 aggregate 只有第一个可绑定
- 根因：缺少"record → [mappers]"多对多映射结构

### 4.3 窗口/聚合语义未定义清楚

- 环形缓冲窗口扫描在回绕后坍缩（已验证）
- 聚合跨调用累积 + 全窗口重扫（二次增长，已验证）
- 聚合结果复用 stream 列偏移存储（列数不足即 OOB）
- LIMIT 正/负的方向语义与文档不符

### 4.4 错误处理策略与安全边界冲突

- AGENTS.md 声明"unwrap() 用于内部假设"，但多处 unwrap 作用于**外部输入**（SQL 列名、用户 data_len、除零）→ release 下 panic=abort 直接杀进程
- Java 宿主中一次非法 SQL/数据即可终止整个 JVM

### 4.5 生命周期管理缺失

- stop() 空实现；所有堆资源永不释放（有意的进程级单例，但无文档化约束）
- 动态 def_mapper 泄漏 LLVM 引擎

## 5. 架构演进建议

| 阶段 | 措施 |
|------|------|
| 短期 | 修复窗口索引算术（first_idx=(count-M)%M + 模运算迭代）；聚合独立偏移缓冲 + 初值初始化；身份体系改为 record→Vec<Mapper> |
| 中期 | 外部输入全校验（data_len、列名、除零）；替换 SimpleU16Map 的 UB 别名；stop() 释放资源 |
| 长期 | 若需多 mapper/多实例/重启能力 → 重写为正常 Rust 所有权 + 内部 Arc 状态（保留 JIT 与环形缓冲设计） |

## 6. 架构评分（1-10）

- 层次清晰度：7（分层明确但全局状态破坏封装）
- 模块独立性：5（模块间经裸指针全局耦合）
- 扩展性：3（多 mapper/多聚合/多实例均受限）
- 安全性：3（UB/panic 多处）
- 性能设计：8（JIT+环形缓冲思路正确，实现有缺陷）
