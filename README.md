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

- Low-latency processing through JIT compilation
- Memory-efficient storage with configurable buffer sizes
- Fixed-size data constraints (512-byte maximum records)
- Optimized for high-throughput streaming scenarios

## Dependencies

- LLVM 19 (for JIT compilation)
- Rust 2021 edition
- Various crates for configuration, logging, and cross-language bindings