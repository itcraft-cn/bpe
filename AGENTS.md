# Bamboo Pipe Engine (BPE)

Streaming data processing engine with SQL-like queries and LLVM JIT compilation.

## Build Commands

```bash
# Debug build (default)
./build.sh 0
cargo build

# Release build (links LLVM-19)
./build.sh 1
RUSTFLAGS='-lLLVM-19' cargo build --release

# Full build (includes Java/Python wrappers)
./build.sh 2  # debug
./build.sh 3  # release
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

There are no specific lint commands configured in the existing project configuration.

## AI Guide

### Code Style Guidelines

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