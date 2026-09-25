use crate::{
    agg_func::{
        f_avg_d, f_avg_l, f_count_l, f_first_d, f_first_l, f_last_d, f_last_l, f_max_d_d,
        f_max_d_l, f_max_l, f_min_d_d, f_min_d_l, f_min_l, f_stddev_finalize, f_stddev_step,
        f_sum_d_d, f_sum_d_l, f_sum_l, f_var_finalize,
    },
    aux::{fetch_ptr, fill_ptr, SimpleU16Map},
    callback::FnHolder,
    consts::{FIELD_SIZE, U8_DATA_MAX_SIZE},
    data::{ColumnType, Record},
    element::Element,
    error::ParseSqlError,
    exec::{create_executor, Executor},
    func_enum::SupportFunc,
    id::next_aggregate_id,
    param::CallbackParams,
    sql::{
        base::{parse_options, ParsedSql},
        select::parse_select,
    },
};
use globalvar::{def_global_ptr, get_global, get_global_mut};
use inkwell::execution_engine::JitFunction;
use std::alloc::{self, Layout};

/// Base offset of the variance/stddev Welford state area inside the aggregate
/// result buffer. The output slots (col_idx * FIELD_SIZE) stay untouched, so the
/// callback layout is unchanged.
const AGG_STATE_BASE: usize = 4096;
/// Per-function Welford state size: [count: f64][mean: f64][m2: f64]
const AGG_STATE_SIZE: usize = 24;

static mut PTR_AGGREGATE_MAP: u64 = 0;
static mut PTR_PARSE_OPTIONS: u64 = 0;
static mut PTR_VAL_DATA_REF: u64 = 0;

/// Initializes the aggregate system by setting up global pointers for:
/// - Aggregate map to store aggregate definitions
/// - Parse options for SQL parsing
/// - Value data reference for aggregate computations
pub(crate) fn init_aggregate() {
    unsafe {
        PTR_AGGREGATE_MAP = def_global_ptr(SimpleU16Map::new()); // Initialize map to store aggregates
        PTR_PARSE_OPTIONS = def_global_ptr(parse_options()); // Initialize SQL parsing options
                                                             // Allocate memory for aggregate data reference
        PTR_VAL_DATA_REF =
            alloc::alloc(Layout::from_size_align(U8_DATA_MAX_SIZE, 1).unwrap()) as u64;
    }
}

/// Defines an aggregate function by parsing the SQL query, generating the aggregate structure,
/// and storing it in the aggregate map for later use.
/// Returns Some(1) if successful, or None if the aggregate could not be created.
pub(crate) fn define_aggregate(sql: &str, func_holder: FnHolder) -> Option<u16> {
    let options = get_global(unsafe { PTR_PARSE_OPTIONS }); // Get global parse options
    if let Some(parsed_sql) = parse_select(sql, options) {
        // Parse the SQL query
        let rs = gen_aggregate(&parsed_sql); // Generate aggregate structure
        if let Ok(aggregate) = rs {
            let map = get_global_mut::<SimpleU16Map>(unsafe { PTR_AGGREGATE_MAP }); // Get aggregate map
            let agg_id = aggregate.id(); // capture real aggregate id
                                          // Store the aggregate with its function holder in the map
            map.insert(agg_id, WrappedAggregate::new(aggregate, func_holder));
            Some(agg_id) // Return the real aggregate id
        } else {
            log::warn!("{:?}", rs.err()); // Log error if aggregate generation failed
            None
        }
    } else {
        None // Return None if SQL parsing failed
    }
}

/// JIT variant of `call_aggregate`: uses the compiled aggregate kernel when
/// available (falls back to the interpreter otherwise).
pub(crate) fn call_aggregate_jit(
    wrapped: &WrappedAggregate,
    jit: Option<JitFunction<'static, crate::jit::aggregate::AggFunc>>,
    offsets: &[usize],
    param: CallbackParams,
) {
    let id = wrapped.aggregate().stream_id();
    let Some(stream) = Record::get_record(id) else {
        log::warn!("failed to find stream by id[{id}]");
        return;
    };
    let aggregate_data_ptr = unsafe { PTR_VAL_DATA_REF } as *mut u8;
    let u8_ptr = param.u8_ptr();
    let size = param.size();
    let step = param.step();
    let out_size = if size > 0 { 1 } else { 0 };
    if let Some(jit) = jit {
        // kernel handles count==0 (initial values) and computes results otherwise
        unsafe { jit.call(u8_ptr, size, step, aggregate_data_ptr) };
    } else if size > 0 {
        if !init_data(aggregate_data_ptr, wrapped) {
            return;
        }
        let p = CallbackParams::new(u8_ptr, 0, 0, size, step);
        compute_data(aggregate_data_ptr, wrapped, stream, p, offsets);
    } else {
        if !init_data(aggregate_data_ptr, wrapped) {
            return;
        }
    }
    // step must be the real row stride (FIELD_SIZE): the egress channel copies
    // size*step bytes, so a wrong stride silently truncates the payload
    crate::egress::dispatch(
        wrapped.fn_holder(),
        aggregate_data_ptr,
        1,
        0,
        out_size,
        FIELD_SIZE,
        0,
        0,
    );
}

/// Initializes aggregate data by setting up initial values for each executor in the aggregate.
/// Results are stored at dense offsets (col_idx * FIELD_SIZE), independent of the stream layout.
/// Returns true if all initializations succeed, false if any fail.
#[inline]
pub(crate) fn init_data(aggregate_data_ptr: *mut u8, wrapped: &WrappedAggregate) -> bool {
    // Iterate through each executor in the aggregate with its column index
    for (col_idx, executor) in wrapped.aggregate().executors().iter().enumerate() {
        // Set up the initial value for this executor
        let rs = setup_init_val(aggregate_data_ptr, col_idx, executor);
        if rs.is_err() {
            log::warn!("hit error: {:?}", rs.err()); // Log error if initialization failed
            return false; // Return false to indicate failure
        }
    }
    true // Return true to indicate all initializations succeeded
}

#[inline]
fn setup_init_val(
    aggregate_data_ptr: *mut u8,
    col_idx: usize,
    executor: &Executor,
) -> Result<(), String> {
    let offset = col_idx * FIELD_SIZE;
    match executor {
        Executor::ConstLong(v) => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
            Ok(())
        }
        Executor::ConstDouble(v) => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
            Ok(())
        }
        Executor::Compute(func, _) => {
            init_for_some_func(func, aggregate_data_ptr, offset);
            Ok(())
        }
        _ => {
            log::warn!(
                "expr in top level just support aggregate func, this is not aggregate func:{executor:?}"
            );
            Err(String::from("not aggregate func"))
        }
    }
}

/// Writes the initial value for each aggregate function into the result buffer.
/// All functions are initialized so the callback never observes uninitialized memory.
#[inline]
fn init_for_some_func(func: &SupportFunc, aggregate_data_ptr: *mut u8, offset: usize) {
    match func {
        SupportFunc::MaxL => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, i64::MIN);
        }
        SupportFunc::MinL => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, i64::MAX);
        }
        SupportFunc::MaxD => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, f64::MIN);
        }
        SupportFunc::MinD => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, f64::MAX);
        }
        SupportFunc::SumL => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, 0_i64);
        }
        SupportFunc::SumD => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, 0.0_f64);
        }
        SupportFunc::Count => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, 0_i64);
        }
        SupportFunc::Avg => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, 0.0_f64);
        }
        SupportFunc::FirstL | SupportFunc::FirstD | SupportFunc::LastL | SupportFunc::LastD => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, 0_i64);
        }
        SupportFunc::Stddev | SupportFunc::StddevSamp | SupportFunc::Variance | SupportFunc::VarSamp => {
            // zero the Welford state area [count, mean, m2]
            let state = unsafe {
                aggregate_data_ptr.add(AGG_STATE_BASE + (offset / FIELD_SIZE) * AGG_STATE_SIZE)
            };
            fill_ptr(state, 0.0_f64);
            fill_ptr(unsafe { state.add(8) }, 0.0_f64);
            fill_ptr(unsafe { state.add(16) }, 0.0_f64);
        }
        _ => {}
    }
}

/// Computes aggregate data by processing each executor in the aggregate.
/// Results are stored at dense offsets (col_idx * FIELD_SIZE), independent of the stream layout.
/// Handles constant values, single-argument functions (First/Last), and multi-argument functions.
#[inline]
pub(crate) fn compute_data(
    aggregate_data_ptr: *mut u8, // Pointer to memory where aggregate data is stored
    wrapped: &WrappedAggregate,  // The wrapped aggregate structure
    stream: &Record,             // The stream record containing column information
    param: CallbackParams,       // Parameters for computation
    offsets: &[usize],           // stream field id -> mapper output offset
) {
    // Create a wrapped parameter structure for aggregate computation
    let wrapped_agg_param = WrappedAggParam::new(aggregate_data_ptr, stream);
    // Process each executor in the aggregate with its column index
    for (col_idx, executor) in wrapped.aggregate().executors().iter().enumerate() {
        // Calculate the memory offset for this column
        let offset = col_idx * FIELD_SIZE;
        match executor {
            Executor::ConstLong(v) => {
                // Handle constant long value by filling the memory at offset
                fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
            }
            Executor::ConstDouble(v) => {
                // Handle constant double value by filling the memory at offset
                fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
            }
            Executor::Compute(func, executors) => {
                // Get the number of executors for this function
                let executor_size = executors.executor_size();
                if executor_size != 1 {
                    // Log warning if function doesn't support the number of arguments
                    log::warn!("aggregate func[{func:?}] just support one argument, here is {executor_size:?} executors");
                    continue; // Continue early if argument count is incorrect
                }
                match func {
                    SupportFunc::FirstL | SupportFunc::FirstD => {
                        // Handle First functions: get the first value
                        let sub_executor = executors.index_of(0); // Get the first sub-executor
                        let element = fetch_arg_val(param.u8_ptr(), sub_executor, stream, offsets); // Get value
                        call_once_compute(&wrapped_agg_param, func, element, offset);
                        // Compute
                    }
                    SupportFunc::LastL | SupportFunc::LastD => {
                        // Handle Last functions: get the last value
                        let sub_executor = executors.index_of(0); // Get the first sub-executor
                        let last = param.size() - 1; // Calculate index of last element

                        // Get pointer to the last data element
                        let sub_data = unsafe { param.u8_ptr().add(last * param.step()) };
                        let element = fetch_arg_val(sub_data, sub_executor, stream, offsets); // Get value
                        call_once_compute(&wrapped_agg_param, func, element, offset);
                        // Compute
                    }
                    _ => {
                        loop_compute(aggregate_data_ptr, stream, col_idx, executor, &param, offsets); // Handle other functions
                        finalize_func(func, aggregate_data_ptr, col_idx); // write back variance/stddev results
                    }
                }
            }
            _ => {}
        }
    }
}

fn call_once_compute(
    wrapped_agg_param: &WrappedAggParam,
    func: &SupportFunc,
    element: Element,
    offset: usize,
) {
    choose_func(func, element, wrapped_agg_param, offset, 0);
}

/// Writes back the final variance/stddev result from the Welford state area
/// into the dense output slot after all records of the window are processed.
fn finalize_func(func: &SupportFunc, aggregate_data_ptr: *mut u8, col_idx: usize) {
    let offset = col_idx * FIELD_SIZE;
    let state =
        unsafe { aggregate_data_ptr.add(AGG_STATE_BASE + col_idx * AGG_STATE_SIZE) };
    match func {
        SupportFunc::Stddev => {
            f_stddev_finalize(unsafe { aggregate_data_ptr.add(offset) }, state, false)
        }
        SupportFunc::StddevSamp => {
            f_stddev_finalize(unsafe { aggregate_data_ptr.add(offset) }, state, true)
        }
        SupportFunc::Variance => {
            f_var_finalize(unsafe { aggregate_data_ptr.add(offset) }, state, false)
        }
        SupportFunc::VarSamp => {
            f_var_finalize(unsafe { aggregate_data_ptr.add(offset) }, state, true)
        }
        _ => {}
    }
}

fn loop_compute(
    aggregate_data_ptr: *mut u8,
    stream: &Record,
    col_idx: usize,
    executor: &Executor,
    param: &CallbackParams,
    offsets: &[usize],
) {
    let u8_ptr = param.u8_ptr();
    let size = param.size();
    let wrapped_agg_param = WrappedAggParam::new(aggregate_data_ptr, stream);
    for data_idx in 0..size {
        compute(
            col_idx,
            &wrapped_agg_param,
            executor,
            unsafe { u8_ptr.add(data_idx * param.step()) },
            data_idx,
            offsets,
        );
    }
}

#[inline]
fn compute(
    col_idx: usize,
    wrapped_agg_param: &WrappedAggParam,
    executor: &Executor,
    sub_data: *const u8,
    data_idx: usize,
    offsets: &[usize],
) {
    let stream = wrapped_agg_param.stream();
    let offset = col_idx * FIELD_SIZE;
    match executor {
        Executor::Compute(func, executors) => {
            let sub_executor = executors.index_of(0);
            let element = fetch_arg_val(sub_data, sub_executor, stream, offsets);
            choose_func(func, element, wrapped_agg_param, offset, data_idx);
        }
        _ => {
            log::warn!(
                "expr in top level just support aggregate func, this is not aggregate func:{executor:?}"
            );
        }
    }
}

#[inline]
fn choose_func(
    func: &SupportFunc,
    element: Element,
    wrapped_agg_param: &WrappedAggParam,
    offset: usize,
    data_idx: usize,
) {
    let aggregate_data_ptr = wrapped_agg_param.aggregate_data_ptr();
    match func {
        SupportFunc::MaxL => match &element {
            Element::Long(v) => {
                f_max_l(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        SupportFunc::MinL => match &element {
            Element::Long(v) => {
                f_min_l(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        SupportFunc::SumL => match &element {
            Element::Long(v) => {
                f_sum_l(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        SupportFunc::Count => {
            // count works on any column type (value is ignored)
            f_count_l(aggregate_data_ptr, offset);
        }
        SupportFunc::MaxD => match &element {
            Element::Long(v) => {
                f_max_d_l(aggregate_data_ptr, offset, v);
            }
            Element::Double(v) => {
                f_max_d_d(aggregate_data_ptr, offset, v);
            }
        },
        SupportFunc::MinD => match &element {
            Element::Long(v) => {
                f_min_d_l(aggregate_data_ptr, offset, v);
            }
            Element::Double(v) => {
                f_min_d_d(aggregate_data_ptr, offset, v);
            }
        },
        SupportFunc::SumD => match &element {
            Element::Long(v) => {
                f_sum_d_l(aggregate_data_ptr, offset, v);
            }
            Element::Double(v) => {
                f_sum_d_d(aggregate_data_ptr, offset, v);
            }
        },
        SupportFunc::Avg => match &element {
            Element::Long(v) => {
                f_avg_l(aggregate_data_ptr, offset, v, data_idx);
            }
            Element::Double(v) => {
                f_avg_d(aggregate_data_ptr, offset, v, data_idx);
            }
        },
        SupportFunc::Stddev | SupportFunc::StddevSamp | SupportFunc::Variance | SupportFunc::VarSamp => {
            // Welford step; input is converted to f64, final result is written back by finalize_func
            let state = unsafe {
                aggregate_data_ptr.add(AGG_STATE_BASE + (offset / FIELD_SIZE) * AGG_STATE_SIZE)
            };
            let x = match &element {
                Element::Long(v) => *v as f64,
                Element::Double(v) => *v,
            };
            f_stddev_step(state, x);
        }
        SupportFunc::FirstL => match &element {
            Element::Long(v) => {
                f_first_l(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        SupportFunc::FirstD => {
            let v = match &element {
                Element::Long(v) => *v as f64,
                Element::Double(v) => *v,
            };
            f_first_d(aggregate_data_ptr, offset, &v);
        }
        SupportFunc::LastL => match &element {
            Element::Long(v) => {
                f_last_l(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        SupportFunc::LastD => {
            let v = match &element {
                Element::Long(v) => *v as f64,
                Element::Double(v) => *v,
            };
            f_last_d(aggregate_data_ptr, offset, &v);
        }
        _ => {
            log::warn!("unsupported function: {:?}-{:?}", func, &element);
        }
    };
}

fn fetch_arg_val(sub_data: *const u8, executor: &Executor, stream: &Record, offsets: &[usize]) -> Element {    match executor {
        Executor::Fetch(_record_id, field_id) => {
            // column type from the stream layout, value position from the mapper output offsets
            let column = stream.column(*field_id);
            let off = *offsets.get(*field_id as usize).unwrap_or(&usize::MAX);
            if off == usize::MAX {
                log::warn!(
                    "aggregate field id [{field_id}] is not selected by the mapper, read as 0"
                );
                Element::Long(0)
            } else {
                match column.data_type() {
                    ColumnType::Long => {
                        Element::Long(unsafe { fetch_ptr(sub_data.add(off)) })
                    }
                    ColumnType::Double => {
                        Element::Double(unsafe { fetch_ptr(sub_data.add(off)) })
                    }
                }
            }
        }
        _ => {
            log::warn!("just support fetch, here is {executor:?} executor");
            Element::Long(0)
        }
    }
}

pub(crate) fn search_aggregate(id: u16) -> Option<&'static WrappedAggregate> {
    let map = get_global::<SimpleU16Map>(unsafe { PTR_AGGREGATE_MAP });
    map.get(id)
}

pub(crate) fn gen_aggregate(parsed_sql: &ParsedSql) -> Result<Aggregate, ParseSqlError> {
    let fields = parsed_sql.fields();
    if fields.is_empty() {
        return Err(ParseSqlError::new(String::from("no field in aggregate")));
    }
    if parsed_sql.records().len() != 1 {
        return Err(ParseSqlError::new(String::from(
            "only support one record in aggregate",
        )));
    }
    let opt_stream = Record::get_record(parsed_sql.records()[0]);
    if opt_stream.is_none() {
        return Err(ParseSqlError::new(format!(
            "stream {} not found",
            parsed_sql.records()[0]
        )));
    }
    let rs = create_executor(parsed_sql.records(), fields);
    if let Ok(executors) = rs {
        let id = next_aggregate_id();
        Ok(Aggregate {
            id,
            stream_id: opt_stream.unwrap().id(),
            executors,
        })
    } else {
        Err(ParseSqlError::new(String::from(
            "fail to create executors for aggregate",
        )))
    }
}

pub(crate) struct Aggregate {
    id: u16,
    stream_id: u16,
    executors: Vec<Executor>,
}
impl Aggregate {
    fn id(&self) -> u16 {
        self.id
    }
    pub(crate) fn stream_id(&self) -> u16 {
        self.stream_id
    }
    pub(crate) fn executors(&self) -> &Vec<Executor> {
        &self.executors
    }
}

pub(crate) struct WrappedAggregate {
    aggregate: Aggregate,
    fn_holder: FnHolder,
}
impl WrappedAggregate {
    pub(crate) fn new(aggregate: Aggregate, fn_holder: FnHolder) -> Self {
        WrappedAggregate {
            aggregate,
            fn_holder,
        }
    }
    pub(crate) fn aggregate(&self) -> &Aggregate {
        &self.aggregate
    }
    pub(crate) fn fn_holder(&self) -> &FnHolder {
        &self.fn_holder
    }
}

struct WrappedAggParam<'a> {
    aggregate_data_ptr: *mut u8,
    stream: &'a Record,
}
impl<'a> WrappedAggParam<'a> {
    fn new(aggregate_data_ptr: *mut u8, stream: &'a Record) -> Self {
        WrappedAggParam {
            aggregate_data_ptr,
            stream,
        }
    }

    fn aggregate_data_ptr(&self) -> *mut u8 {
        self.aggregate_data_ptr
    }

    fn stream(&self) -> &'a Record {
        self.stream
    }
}
