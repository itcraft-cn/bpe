# Bamboo Pipe Engine (BPE)

一个用Rust编写的高性能流数据处理引擎，支持多语言。BPE提供基于SQL查询接口的实时复杂事件处理(CEP)功能和JIT编译。

## 概述

BPE (Bamboo Pipe Engine) 是一个流数据处理系统，使用类SQL查询实现数据流的实时分析。该引擎以性能为设计重点，使用基于LLVM的JIT编译实现最佳执行速度。

## 架构

BPE项目由多个互连模块组成：

### 核心组件

1. **bpe-core** - 提供核心流数据处理引擎的主要Rust库
   - 实时流处理能力
   - 带JIT编译的基于SQL的查询接口
   - 内存高效的环形缓冲区存储
   - 聚合函数(SUM, COUNT, AVG, MIN, MAX, FIRST, LAST)

2. **bpe-java-wrapper (bpe4j)** - BPE的Java JNI包装器
   - 为Rust核心提供Java绑定
   - 使用JNI进行跨语言通信
   - 基于Maven的构建系统

3. **bpe-py-wrapper (bpe4py)** - 使用PyO3的Python包装器
   - 通过PyO3提供Python绑定
   - Wheel分发便于安装
   - 支持Python 3.7+

4. **bpe-sample** - 核心库的使用示例
   - 演示BPE功能的示例实现
   - 使用示例和模式

5. **bpe-test** - 核心库的测试套件
   - 单元测试和集成测试
   - 核心功能验证

## 特性

### 核心引擎
- 使用类SQL语法的实时流处理
- 使用LLVM进行JIT编译以实现高性能
- 内存高效的环形缓冲区存储系统
- 可配置的数据模式，支持Long/Double类型
- 内置聚合函数(SUM, COUNT, AVG, MIN, MAX, FIRST, LAST)
- 自定义函数扩展性

### 多语言支持
- 原生Rust API以实现最大性能
- 通过JNI的Java绑定
- 通过PyO3的Python绑定
- 跨平台兼容性

### 配置
- 环境变量支持(`BPE_HOME`)
- 基于TOML的配置(`cfg/config.toml`)
- 运行时可配置参数:
  - 向量大小(默认: 1MB)
  - 记录大小(默认: 512字节, 最大: 512字节)
  - 开发模式标志
  - 日志目录配置

## 构建项目

项目通过构建脚本支持多种构建模式：

```bash
# 构建模式:
# 0 - 简单构建 (调试)
# 1 - 简单构建 (发布) 
# 2 - 完整构建 (调试) - 包含Java和Python包装器
# 3 - 完整构建 (发布) - 包含Java和Python包装器

./build.sh 0  # 调试构建
./build.sh 1  # 发布构建
./build.sh 2  # 完整调试构建 (含包装器)
./build.sh 3  # 完整发布构建 (含包装器)
```

## 使用

### Rust
```rust
use bpe;

// 初始化引擎
bpe::start();

// 定义传入数据模式
let incoming_id = bpe::def_incoming("sensor_data", vec![
    bpe::Column::new_long("id"),
    bpe::Column::new_double("temperature"),
]);

// 定义流模式
let stream_id = bpe::def_stream("high_temp_alert", vec![
    bpe::Column::new_long("id"),
    bpe::Column::new_double("temperature"),
]);

// 使用SQL定义处理映射器
let mapper_id = bpe::def_mapper("SELECT id, temperature FROM sensor_data WHERE temperature > 30.0", 
    |params| {
        // 处理过滤后的数据
        println!("检测到高温!");
    });

// 向引擎发送数据
let data = bpe::U8Bytes::new_from_vec(incoming_id.unwrap(), 16, vec![/* 数据字节 */]);
bpe::new_data(&data);

// 清理
bpe::stop();
```

### 配置文件 (cfg/config.toml)
```toml
dev_mode = false
vec_size = 1048576
record_size = 512
log_dir = "logs"
```

## 性能特点

- 通过JIT编译实现低延迟处理
- 具有可配置缓冲区大小的内存高效存储
- 固定大小数据约束(512字节最大记录)
- 为高吞吐量流场景优化

## 依赖

- LLVM 19 (用于JIT编译)
- Rust 2021 edition
- 用于配置、日志记录和跨语言绑定的各种包