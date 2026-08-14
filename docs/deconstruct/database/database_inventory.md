# BPE 数据库清单

> 生成日期：2026-08-15（full-analysis / code-deconstruct）

## 结论：本项目无持久化数据库

**BPE 为纯内存流式处理引擎，不涉及数据库。**

### 扫描结果

| 检查项 | 结果 |
|--------|------|
| SQL 文件 (*.sql) | 无 |
| MyBatis/iBATIS mapper | 无（Java 侧 bpe4j 无 ORM） |
| JDBC 连接 | 无 |
| 内嵌 SQL（SELECT/INSERT/UPDATE/DELETE） | 仅引擎自身的流式查询 SQL 字符串（Rust 常量，如 FILTER_SQL/AGGREGATE_SQL），非数据库 SQL |
| 持久化/序列化 | 无（无 serde/rocksdb/sqlite 依赖） |

### 说明

项目中的"SQL"是引擎自有的流式查询语言（经 sql-parse crate 解析，MariaDB 方言子集），
仅支持 SELECT 单表 + WHERE + LIMIT，不连接任何外部数据库。

因此：
- 不生成 ER 图
- 不生成数据库设计文档
- 无 SQL 注入风险（SQL 由引擎内部定义，非用户输入直接拼接）
