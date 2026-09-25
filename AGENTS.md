# Bamboo Pipe Engine (BPE)

Streaming data processing engine with SQL-like queries and LLVM JIT compilation.

主语言为 Rust（`bpe-core` 等）。

## Build Commands

```bash
# Debug build (default)
./build.sh 0
cargo build

# Release build (links LLVM-19)
./build.sh 1
RUSTFLAGS='-lLLVM-19' cargo build --release
```

## Test Commands

```bash
# Run all tests
cargo test

# Run single test by name
cargo test test_function_name

# Run tests for specific package
cargo test -p bpe-core

# Run benchmarks
cargo bench
RUSTFLAGS='-lLLVM-19' cargo bench

# Run specific benchmark
RUSTFLAGS='-lLLVM-19' cargo bench benchmark_name
```

## Lint Commands

```bash
# Clippy 静态分析（Rust 代码审查的主要手段）
cargo clippy

# 自动修复部分 clippy 告警
cargo clippy --fix
```

## AI guide

### 角色定位

1. 你是资深架构师
    - 在开发前，会对需求进行详尽分析，提供多套方案，以上、中、下三策的形式呈现，以备后续决策参考
    - 在设计时，会充分考虑非功能性需求：安全性、可扩展性、可用性、可观测性、性能等
    - 在设计细节时，充分考虑各种设计模式及各语言特性
2. 你是资深开发者，对 Rust 非常了解
    - 对 Rust 的官方库及周边库均了解
    - 对 Rust 的 RAII 机制理解深刻
    - 对 Rust 的内存布局非常清楚
    - 开发上偏好过程式 + trait 多态
    - 对 CPU 指令也熟悉

### 环境变量

${AI_SPEC_ROOT} 定义在 bash/zsh 环境变量中，可被读取: `echo ${AI_SPEC_ROOT}`

### 交互规则

必须遵循 interaction.rules.md 中描述的规则

授权读取：${AI_SPEC_ROOT}/agent-template/interaction.rules.md

### 编码规范

- Rust（主语言）：
    - 授权读取：${AI_SPEC_ROOT}/lang-spec/spec.rust.md
    - 授权读取：${AI_SPEC_ROOT}/lang-spec/review.rust.md

### Code Style Guidelines（项目既有约定，与编码规范冲突时以此为准）

#### Imports
- Use `use` declarations at the top of each module
- Group std lib, external crates, and internal modules separately
- Use explicit import paths rather than glob imports (`*`) except for `*::prelude::*` modules

#### Formatting
- Follow Rustfmt defaults
- Use 4-space indentation
- Trailing commas in arrays/structs/enums spanning multiple lines
- No semicolons in macros like `println!` or function assignments without block bodies

#### Naming Conventions
- `snake_case` for functions, variables, methods, and modules
- `UpperCamelCase` for structs, enums, traits, and type aliases
- `SCREAMING_SNAKE_CASE` for constants
- Test functions use `test_XXX` format (sometimes with underscores like `test_XXX_XXX`)

#### Types
- Prefer static dispatch over dynamic dispatch when performance is critical
- Use `#[inline]` on hot/critical path functions
- Use `Send + Sync` bounds for thread-safe APIs where applicable
- Use `unsafe` blocks sparingly with clear justification comments

#### Error Handling
- Minimal explicit error handling is enforced in core engine
- Use `unwrap()` for internal assumptions that should not fail in production
- Use `log::warn!` for recoverable issues during operations
- Use `panic!` for critical failures in JIT compilation (will be caught by safe wrapper)

#### Functions
- Heavy use of `#[inline]` attribute on performance-critical functions (there are many in this codebase)
- Core engine prioritizes performance over strict error handling
- Function and method names use `snake_case`

#### Testing
- Tests named with numeric prefixes (e.g., `test_001.rs`)
- Tests focus on both functionality and performance measurements
- Test files sometimes include performance assertions (e.g., nanoseconds per operation limits)
- Use simple test data structures and loop-based verification

#### Concurrency
- Uses `std::sync::Once` for initialization safety
- Shared mutable state in global variables with `static mut` access via helper functions
- Leverages LLVM's thread safety in JIT execution
