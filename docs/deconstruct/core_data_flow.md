# Bamboo Pipe Engine - Core Data Flow

## Overview

BPE processes streaming data through a pipeline of: **Definition → Ingestion → JIT Filtering → Callback Execution**.

## Main Data Flow

```mermaid
flowchart TD
    subgraph Init["1. System Initialization"]
        START[start] --> CONFIG[load_config]
        CONFIG --> LOGGER[init_logger]
        LOGGER --> JIT[init_func_generator]
        JIT --> DATA[init_data]
        DATA --> STORE[init_store]
        STORE --> MAPPER[init_mapper]
        MAPPER --> AGG[init_aggregate]
    end

    subgraph Define["2. Definition Phase"]
        DEF_IN[def_incoming] --> REC[Record::insert_record]
        REC --> STORE_REC[Store Record Definition]
        
        DEF_MAP[def_mapper] --> PARSE[parse_select]
        PARSE --> JIT_COMPILE[JIT Compile Filter]
        JIT_COMPILE --> CREATE_EXEC[Create Executors]
        CREATE_EXEC --> STORE_MAP[Store WrappedMapper]
    end

    subgraph Process["3. Data Processing Phase"]
        NEW_DATA[new_data] --> FIND_ARR[find_or_insert_array]
        FIND_ARR --> INSERT[insert into circular buffer]
        INSERT --> CALL_MAP[call_mapper]
        CALL_MAP --> LOOP{loop_filter}
        LOOP --> FILTER{JIT Filter?}
        FILTER -->|pass| FETCH[fetch fields]
        FILTER -->|fail| LOOP
        FETCH --> COPY[copy to output buffer]
        COPY --> LOOP
        LOOP -->|done| CALLBACK[callback]
    end

    subgraph Aggregate["4. Aggregate Phase"]
        DEF_AGG[def_aggregate] --> PARSE_AGG[parse_select]
        PARSE_AGG --> GEN_AGG[gen_aggregate]
        GEN_AGG --> STORE_AGG[Store WrappedAggregate]
        
        CALL_AGG[call_aggregate] --> INIT[init_data]
        INIT --> COMPUTE[compute_data]
        COMPUTE --> AGG_LOOP{More elements?}
        AGG_LOOP --> FETCH_VAL[fetch_arg_val]
        FETCH_VAL --> CHOOSE_FUNC[choose_func: Max/Min/Sum/Avg]
        CHOOSE_FUNC --> AGG_LOOP
        AGG_LOOP -->|done| AGG_CALLBACK[callback]
    end

    Init --> Define
    Define --> Process
    Process --> Aggregate
```

## JIT Filter Compilation Flow

```mermaid
flowchart LR
    subgraph Input
        SQL[SQL WHERE Clause]
    end
    
    subgraph Parsing
        PARSE[parse_exp]
        BIN[Binary Expression]
        ID[Identifier/Column]
        LIT[Literal Value]
    end
    
    subgraph JIT
        LLVM[LLVM IR Generation]
        COMPILE[JIT Compilation]
        NATIVE[Native Code]
    end
    
    subgraph Execution
        FILTER[JIT Filter Function]
        DATA[Data Pointer]
        RESULT[Boolean Result]
    end
    
    SQL --> PARSE
    PARSE --> BIN
    PARSE --> ID
    PARSE --> LIT
    BIN --> LLVM
    ID --> LLVM
    LIT --> LLVM
    LLVM --> COMPILE
    COMPILE --> NATIVE
    NATIVE --> FILTER
    DATA --> FILTER
    FILTER --> RESULT
```

## Circular Buffer Data Flow

```mermaid
flowchart TD
    subgraph Storage["WrappedArray (Circular Buffer)"]
        DATA_PTR[data: *mut u8]
        WALKER[walker: usize]
        MASK[mask: usize]
        STEP[step: usize]
    end
    
    subgraph Insert["Data Insertion"]
        U8BYTES[U8Bytes Input]
        BASE[Calculate base position]
        WRITE[write_data]
        UPDATE[update_walker]
    end
    
    subgraph Read["Data Retrieval"]
        POS[Calculate position]
        SUB[sub_data]
        PTR[Pointer to record]
    end
    
    U8BYTES --> BASE
    BASE --> WRITE
    WRITE --> UPDATE
    UPDATE --> WALKER
    
    WALKER --> POS
    MASK --> POS
    STEP --> POS
    POS --> SUB
    DATA_PTR --> SUB
    SUB --> PTR
```

## Executor Execution Flow

```mermaid
flowchart TD
    subgraph ExecutorTypes["Executor Types"]
        CONST_L[ConstLong: i64]
        CONST_D[ConstDouble: f64]
        FETCH[Fetch: record_id, field_id]
        COMPUTE[Compute: func, executors]
    end
    
    subgraph Execution["Execution"]
        EXEC[executor.fetch]
        VAL[fetch_val]
        FUNC[compute_func]
    end
    
    subgraph Operations["Operations"]
        ADD[_add]
        SUB[_sub]
        MUL[_mul]
        DIV[_div]
        MOD[_mod]
    end
    
    subgraph Result["Result"]
        ELEM[Element]
    end
    
    CONST_L --> EXEC
    CONST_D --> EXEC
    FETCH --> EXEC
    COMPUTE --> EXEC
    
    EXEC --> VAL
    EXEC --> FUNC
    
    FUNC --> ADD
    FUNC --> SUB
    FUNC --> MUL
    FUNC --> DIV
    FUNC --> MOD
    
    VAL --> ELEM
    ADD --> ELEM
    SUB --> ELEM
    MUL --> ELEM
    DIV --> ELEM
    MOD --> ELEM
```

## Key Data Structures

| Structure | Purpose | Size |
|-----------|---------|------|
| `U8Bytes` | Fixed-size data record container | 512 bytes (default) |
| `WrappedArray` | Circular buffer for record storage | Configurable |
| `Record` | Schema definition for data records | Variable |
| `Mapper` | SQL query processor with JIT filter | Variable |
| `Executor` | Expression evaluation tree node | Variable |
| `Element` | Typed value (Long/Double) | 8 bytes |

## Performance Characteristics

- **JIT Filter**: Nanosecond-level latency after compilation
- **Circular Buffer**: O(1) insertion and access
- **Zero-copy**: Direct pointer access to data
- **Cache-friendly**: Contiguous memory layout