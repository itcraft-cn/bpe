# bpe-core

A high-performance streaming data processing engine that implements a SQL-based query system for real-time data processing with JIT compilation capabilities.

## Overview

The bpe-core library is a streaming data processing engine designed to handle continuous data streams with SQL-defined transformations and aggregations. It provides real-time processing capabilities with performance optimizations through LLVM-based JIT compilation.

## Architecture

### Core Components

1. **Core System (`core.rs`)**: Main entry point with initialization functions that coordinate all subsystems
2. **Data Management (`data.rs`)**: Schema definitions and data record handling
3. **Storage System (`store.rs`)**: Circular buffer implementation for efficient memory usage
4. **SQL Processing (`sql/`)**: SQL parsing and execution pipeline
5. **JIT Compilation (`jit/`)**: LLVM-based runtime code generation
6. **Processing Components (`mapper.rs`, `aggregate.rs`)**: Data transformation and aggregation logic

### Data Model

- **U8Bytes**: Binary data container with ID and size metadata
- **Column**: Schema definition with name, type (Long/Double), and memory offset
- **Record**: Data schema with named columns and type information
- **WrappedArray**: Circular buffer storage for streaming data

### Processing Pipeline

1. **Data Ingestion**: Incoming data is stored in circular buffers
2. **SQL Parsing**: SQL statements are parsed into expression trees
3. **JIT Compilation**: Filters and expressions are compiled to native code
4. **Execution**: Processed data triggers user-defined callbacks

## Key Features

### Performance Optimizations
- LLVM-based JIT compilation for filter expressions
- Memory-efficient circular buffer storage
- Fixed-size allocations to minimize allocation overhead
- Direct memory access patterns for optimal performance

### SQL Support
- SELECT statements with filtering capabilities
- Aggregate functions (sum, count, avg, min, max, first, last)
- Custom function support through underscore-prefixed function names
- Column-based data access patterns

### Extensibility
- Callback system for custom processing logic
- FFI support for integration with other systems
- Plugin architecture for custom functions
- Type-safe data processing with Long/Double types

## Design Patterns

### Global State Management
The system uses the `globalvar` crate for shared state across components, enabling efficient access to configuration, storage, and processing components.

### Memory Management
- Raw pointer operations for high-performance data access
- Fixed-size allocations to prevent fragmentation
- Unsafe code blocks with careful bounds checking

### Processing Model
- Stream-based processing with continuous data flow
- Event-driven callbacks for processed results
- Configurable buffer sizes and processing parameters

## Security Considerations

- Extensive use of unsafe code requires careful validation
- JIT compilation could present security risks with untrusted input
- Input validation is implemented for data records
- Memory bounds checking is critical for safe operation

## Performance Characteristics

- Low-latency processing through JIT compilation
- Memory-efficient storage with circular buffers
- Fixed-size data constraints (512-byte maximum)
- Optimized for high-throughput streaming scenarios

## Configuration Options

The library supports several configuration options through environment variables and TOML configuration files:

### Environment Variables
- `BPE_HOME`: Base directory for configuration files (default: current directory)

### Configuration File
- Configuration file path: `cfg/config.toml`
- Supported configuration keys:
  - `dev_mode`: Development mode flag (boolean)
  - `vec_size`: Size of internal vectors (default: 1048576 bytes)
  - `record_size`: Size of individual records (default: 512 bytes, maximum configurable: 512 bytes)
  - `log_dir`: Directory for log files

### Default Values
- `U8_DATA_MAX_SIZE`: 512 bytes (hardcoded maximum size for data records)
- `DEFAULT_VEC_SIZE`: 1048576 bytes (1MB) for vector storage
- `DEFAULT_RECORD_SIZE`: Same as U8_DATA_MAX_SIZE (512 bytes)
- `DEFAULT_SELECT_SIZE`: 10 records for default SELECT operations
- `FIELD_SIZE`: 8 bytes (size of Long/Double fields)

## Limitations

- Complex architecture may require significant maintenance effort
- Heavy reliance on global state may complicate testing
- Extensive use of unsafe code requires careful review

## Usage Patterns

The library is designed for real-time stream processing scenarios where:
- High throughput is required
- SQL-based transformations are beneficial
- Low-latency processing is critical
- Memory efficiency is important

## Integration Points

- Custom callback functions via `def_mapper` and `def_aggregate`
- FFI support through the `FfiFunc` trait
- Configuration via TOML files
- External system integration through callback mechanisms