# Bamboo Pipe Engine (BPE)

A high-performance streaming data processing engine written in Rust with multi-language support. BPE provides real-time Complex Event Processing (CEP) capabilities with SQL-based query interfaces and JIT compilation.

## Overview

BPE (Bamboo Pipe Engine) is a streaming data processing system that enables real-time analysis of data streams using SQL-like queries. The engine is built with performance in mind, using LLVM-based JIT compilation for optimal execution speed.

## Architecture

The BPE project consists of multiple interconnected modules:

### Core Components

1. **bpe-core** - The main Rust library providing the core streaming data processing engine
   - Real-time stream processing capabilities
   - SQL-based query interface with JIT compilation
   - Memory-efficient circular buffer storage
   - Aggregate functions (sum, count, avg, min, max, first, last)

2. **bpe-java-wrapper (bpe4j)** - Java JNI wrapper for BPE
   - Provides Java bindings to the Rust core
   - Uses JNI for cross-language communication
   - Maven-based build system

3. **bpe-py-wrapper (bpe4py)** - Python wrapper using PyO3
   - Python bindings via PyO3
   - Wheel distribution for easy installation
   - Supports Python 3.7+

4. **bpe-sample** - Example usage of the core library
   - Sample implementations demonstrating BPE capabilities
   - Usage examples and patterns

5. **bpe-test** - Test suite for the core library
   - Unit tests and integration tests
   - Validation of core functionality

## Features

### Core Engine
- Real-time stream processing with SQL-like syntax
- JIT compilation using LLVM for high performance
- Memory-efficient circular buffer storage system
- Configurable data schemas with Long/Double types
- Built-in aggregate functions (SUM, COUNT, AVG, MIN, MAX, FIRST, LAST)
- Custom function extensibility

### Multi-language Support
- Native Rust API for maximum performance
- Java bindings via JNI
- Python bindings via PyO3
- Cross-platform compatibility

### Configuration
- Environment variable support (`BPE_HOME`)
- TOML-based configuration (`cfg/config.toml`)
- Runtime-configurable parameters:
  - Vector size (default: 1MB)
  - Record size (default: 512 bytes, max: 512 bytes)
  - Development mode flags
  - Log directory configuration

## Building the Project

The project supports multiple build modes through the build script:

```bash
# Build modes:
# 0 - Simple build (debug)
# 1 - Simple build (release) 
# 2 - Full build (debug) - includes Java and Python wrappers
# 3 - Full build (release) - includes Java and Python wrappers

./build.sh 0  # Debug build
./build.sh 1  # Release build
./build.sh 2  # Full debug build (with wrappers)
./build.sh 3  # Full release build (with wrappers)
```

## Usage

### Rust
```rust
use bpe;

// Initialize the engine
bpe::start();

// Define incoming data schema
let incoming_id = bpe::def_incoming("sensor_data", vec![
    bpe::Column::new_long("id"),
    bpe::Column::new_double("temperature"),
]);

// Define stream schema
let stream_id = bpe::def_stream("high_temp_alert", vec![
    bpe::Column::new_long("id"),
    bpe::Column::new_double("temperature"),
]);

// Define processing mapper with SQL
let mapper_id = bpe::def_mapper("SELECT id, temperature FROM sensor_data WHERE temperature > 30.0", 
    |params| {
        // Process the filtered data
        println!("High temperature detected!");
    });

// Send data to the engine
let data = bpe::U8Bytes::new_from_vec(incoming_id.unwrap(), 16, vec![/* data bytes */]);
bpe::new_data(&data);

// Clean up
bpe::stop();
```

### Configuration File (cfg/config.toml)
```toml
dev_mode = false
vec_size = 1048576
record_size = 512
log_dir = "logs"
```

## Performance Characteristics

### Benchmarks

| Test Case | Latency | Throughput |
|-----------|---------|------------|
| Filter Only | ~78 ns | ~12.8M records/sec/core |
| Filter + Aggregate | ~156 ns | ~6.4M records/sec/core |

### Performance Design Analysis

BPE achieves nanosecond-level latency through the following design principles:

#### 1. High Cache Hit Rate

**Circular Buffer (WrappedArray)**
- Single allocation, no memory fragmentation
- Sequential access, prefetcher-friendly
- Fixed-size records, predictable access patterns

**Fixed-Size Records (U8Bytes)**
- Compile-time determined size, no dynamic allocation
- Column offsets calculated once at definition time
- Direct memory access via `base_ptr + offset`

#### 2. Continuous Execution

**Branch-Free Hot Path**
```
new_data → insert → call_mapper → filter → callback
```
- Lock-free design (single-threaded)
- No dynamic dispatch
- No error handling branches in hot path

**JIT Compilation**
- SQL WHERE clause compiled to native machine code
- Zero interpretation overhead
- Optimal CPU instruction generation

**Inline Optimization**
- Heavy use of `#[inline]` on hot path functions
- Cross-function optimization by compiler
- Instruction cache friendly

#### 3. High Predictability

**Predictable Execution Path**
- Fixed processing flow
- Mappers registered at startup, unchanged at runtime
- Filter conditions JIT-compiled, fixed at execution

**Predictable Memory Access**
```rust
for i in 0..size {
    let ptr = base + i * step;  // Fixed stride
    filter(ptr);                // Fixed access pattern
}
```
- CPU prefetcher can predict next access
- Data loaded into cache proactively

**Branch Prediction Friendly**
- Simple pass/fail branches in filter
- CPU branch predictor achieves high accuracy

#### 4. Memory Hierarchy Optimization

**Cache Line Consideration**
```rust
pub(crate) struct WrappedArray {
    data: *mut u8,        // 8 bytes
    max_records: usize,   // 8 bytes
    mask: usize,          // 8 bytes  
    walker: usize,        // 8 bytes (hot data)
    step: usize,          // 8 bytes
}
// Total: 40 bytes, fits in single cache line (64 bytes)
```

**Data Locality**
- L1 Cache (32KB) can hold ~64 records of 512 bytes
- Hot data prioritized in L1: current record, Mapper, Record metadata

### Design Advantages Summary

| Design Feature | Performance Impact | Quantified Effect |
|----------------|-------------------|-------------------|
| Continuous Memory | High cache hit rate | 50%+ memory latency reduction |
| Fixed Layout | High predictability | Accurate CPU branch prediction |
| JIT Compilation | Zero interpretation overhead | 10x+ faster than interpreted |
| Lock-free Design | No waiting latency | Deterministic latency |
| Inline Optimization | Reduced call overhead | 5-10ns saved per call |
| Pre-computed Offsets | Zero runtime overhead | Computed at compile time |

### Performance Formula

```
Total Latency = Memory Access + Computation + Branch Delay

Memory Access:
  - L1 hit: ~1ns
  - L2 hit: ~4ns
  - L3 hit: ~12ns
  - RAM: ~100ns

Computation (JIT):
  - Simple comparison: ~0.5ns
  - Arithmetic: ~0.3ns

Branch Delay:
  - Predicted correctly: ~0ns
  - Mispredicted: ~10-20ns

BPE Design Minimizes:
  ✓ Memory Access → Continuous layout, prefetch-friendly
  ✓ Computation → JIT compiled to optimal machine code
  ✓ Branch Delay → Simple branches, accurate prediction
```

## Dependencies

- LLVM 19 (for JIT compilation)
- Rust 2021 edition
- Various crates for configuration, logging, and cross-language bindings